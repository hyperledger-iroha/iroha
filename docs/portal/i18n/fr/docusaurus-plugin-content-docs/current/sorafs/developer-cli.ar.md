---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-cli
title : كتيب وصفات CLI لـ SoraFS
sidebar_label : et CLI
description: شرح موجّه للمهام لسطح `sorafs_cli` الموحّد.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/developer/cli.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

سطح `sorafs_cli` الموحّد (الموفر من crate `sorafs_car` مع تمكين ميزة `cli`) يعرض كل خطوة مطلوبة Utilisez le code SoraFS. استخدم هذا الكتيب للانتقال مباشرةً إلى مسارات العمل الشائعة؛ واقرنه بخط أنابيب المانيفست ودلائل تشغيل المُنسِّق للحصول على السياق التشغيلي.

## تغليف الحمولات

Utilisez `car pack` pour les morceaux CAR et les morceaux. Il s'agit d'un chunker SF-1 pour vous aider à le faire.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Nom du chunker : `sorafs.sf1@1.0.0`.
- Utilisez des sommes de contrôle et des sommes de contrôle.
- JSON digère les fragments et les fragments ainsi que le CID des fragments avant de les ajouter.

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

- تربط خيارات `--pin-*` مباشرةً بحقوقول `PinPolicy` ضمن `sorafs_manifest::ManifestBuilder`.
- `--chunk-plan` est utilisé pour la CLI en utilisant Digest SHA3 pour un chunk dans la version ultérieure. وإلا فإنه يعيد استخدام digest المضمّن في الملخص.
- JSON utilise Norito pour les diffs.

## توقيع المانيفستات دون مفاتيح طويلة الأمد

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```- يقبل رموزًا مضمنة، ومتغيرات بيئة، أو مصادر قائمة على ملفات.
- يضيف بيانات منشأ (`token_source`, `token_hash_hex`, digest الـ chunk) pour JWT الخام ما لم يكن `--include-token=true`.
- Il s'agit d'un CI : il s'agit de OIDC et d'actions GitHub, comme `--identity-token-provider=github-actions`.

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

- يجري فك ترميز Norito لأدلة alias ويتحقق من تطابقها مع digest المانيفست قبل POST إلى Torii.
- Je digère SHA3 pour un morceau avec les détails de votre projet.
- تلتقط ملخصات الاستجابة حالة HTTP والرؤوس وحمولات السجل للتدقيق لاحقًا.

## التحقق من محتوى CAR والأدلة

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- يعيد بناء شجرة PoR ويقارن digère الحمولة بملخص المانيفست.
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
- ينهي بخروج غير صفري عندما تُبلغ البوابة عن إخفاقات أو عندما ترفض عملية التحقق المحلية من PoR (عبر `--por-root-hex`). اضبط العتبات عبر `--max-failures` et `--max-verification-failures` لتجارب التدريب.
- يدعم PoR حاليًا؛ Le PDP et le PoTR sont également compatibles avec le SF-13/SF-14.
- يكتب `--governance-evidence-dir` الملخص المُنسّق والبيانات الوصفية (طابع الزمني، إصدار CLI، عنوان URL للبوابة، digest المانيفست) ونسخة من المانيفست في الدليل المحدد بحيث يمكن لحزم الحوكمة دليل تدفق الأدلة دون إعادة تشغيل التنفيذ.

## مراجع إضافية

- `docs/source/sorafs_cli.md` — توثيق شامل للأعلام.
- `docs/source/sorafs_proof_streaming.md` — مخطط تليمترية الأدلة وقالب لوحة Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — Utilisé pour le chunking et la CAR.