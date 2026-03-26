---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-cli
título: كتيب وصفات CLI para SoraFS
sidebar_label: CLI
description: شرح موجّه للمهام لسطح `sorafs_cli` الموحّد.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/developer/cli.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

A caixa `sorafs_cli` (a caixa `sorafs_car` é a caixa `cli`) é uma caixa de papelão Verifique o SoraFS. Faça o download do seu cartão de crédito para obter mais informações واقرنه بخط أنابيب المانيفست ودلائل تشغيل المُنسِّق للحصول على السياق التشغيلي.

## تغليف الحمولات

Use `car pack` para quebrar o carro e pedaços de carro. Você pode usar o chunker SF-1 para obter mais informações.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Código do chunker: `sorafs.sf1@1.0.0`.
- تُستعرض مُدخلات الدلائل بترتيب معجمي حتى تبقى checksums ثابتة عبر المنصات.
- يتضمن ملخص JSON digests الحمولة وبيانات وصفية لكل chunk وCID الجذري المعترف به من السجل والمُنسِّق.

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

- Você pode usar o `--pin-*` para substituir o `PinPolicy` pelo `sorafs_manifest::ManifestBuilder`.
- وفّر `--chunk-plan` عندما تريد من CLI إعادة احتساب digest SHA3 للـ chunk قبل الإرسال؛ Não há nenhum resumo do resumo do site.
- O JSON do arquivo Norito é definido como diffs para os parâmetros.

## توقيع المانيفستات دون مفاتيح طويلة الأمد

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- يقبل رموزًا مضمنة, ومتغيرات بيئة, أو مصادر قائمة على ملفات.
- O arquivo de resumo (`token_source`, `token_hash_hex`, resumo do pedaço) é o JWT que está no `--include-token=true`.
- Use o CI: Use OIDC no GitHub Actions para usar `--identity-token-provider=github-actions`.

## إرسال المانيفستات إلى Torii

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

- يجري فك ترميز Norito لأدلة alias ويتحقق من تطابقها مع digest المانيفست قبل POST إلى Torii.
- يعيد احتساب digest SHA3 للـ chunk من الخطة لمنع هجمات عدم التطابق.
- تلتقط ملخصات الاستجابة حالة HTTP والرؤوس وحمولات السجل للتدقيق لاحقًا.

## التحقق من محتوى CAR والأدلة

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- يعيد بناء شجرة PoR ويقارن digere الحمولة بملخص المانيفست.
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
```

- يُصدر عناصر NDJSON لكل دليل يتم بثه (يمكن تعطيل الإعادة عبر `--emit-events=false`).
- يجمّع عدادات النجاح/الفشل وهيستوغرامات الكمون والإخفاقات المأخوذة عينات ضمن ملخص JSON بحيث Certifique-se de que o produto esteja funcionando corretamente.
- ينهي بخروج غير صفري عندما تُبلغ البوابة, عن إخفاقات, أو عندما ترفض عملية التحقق المحلية Em PoR (عبر `--por-root-hex`). Verifique se `--max-failures` e `--max-verification-failures` são usados.
- يدعم PoR حاليًا؛ O PDP e o PoTR são responsáveis ​​​​pelo teste de segurança e pelo SF-13/SF-14.
- يكتب `--governance-evidence-dir` الملخص المُنسّق والبيانات الوصفية (الطابع الزمني, إصدار CLI, عنوان URL للبوابة, digest المانيفست). Eu não sei o que fazer.

## مراجع إضافية- `docs/source/sorafs_cli.md` — É um erro de configuração.
- `docs/source/sorafs_proof_streaming.md` — مخطط تليمترية الأدلة وقالب لوحة Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — تعمّق في chunking وتركيب المانيفست ومعالجة CAR.