---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# خطة المعاينة المحكومة بالتحقق من checksum

Você pode fazer isso com a ajuda de uma pessoa que esteja no lugar certo. A soma de verificação é a soma de verificação do CI e da soma de verificação. Para obter mais informações, verifique o SoraFS com o Norito.

## الأهداف

- **عمليات بناء حتمية:** تأكد من أن `npm run build` ينتج مخرجات قابلة لإعادة الإنتاج ويصدر Modelo `build/checksums.sha256`.
- **معاينات متحقق منها:** اشترط أن يصاحب كل أثر معاينة بيان checksum وارفض النشر عندما يفشل التحقق.
- **بيانات وصفية منشورة عبر Norito:** احفظ واصفات المعاينة (بيانات commit الوصفية, digest checksum, ومعرف CID لـ SoraFS) é Norito JSON para que você possa usar o arquivo .
- **أدوات للمشغلين:** وفر سكربت تحقق بخطوة واحدة يمكن للمستهلكين تشغيله محليا (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); Isso significa que você precisa de uma soma de verificação + uma soma de verificação. O código de barras (`npm run serve`) pode ser usado para substituir `docusaurus serve` por meio de um teste. A soma de verificação é uma soma de verificação (a soma de verificação é `npm run serve:verified`).

## Passo 1 — فرض CI

1. Selecione `.github/workflows/docs-portal-preview.yml` como:
   - O `node docs/portal/scripts/write-checksums.mjs` deve ser substituído por Docusaurus (não disponível).
   - ينفذ `cd build && sha256sum -c checksums.sha256` ويفشل المهمة عند عدم التطابق.
   - Construir um build para `artifacts/preview-site.tar.gz`, e uma soma de verificação, e `scripts/generate-preview-descriptor.mjs`, e `scripts/sorafs-package-preview.sh` O JSON (`docs/examples/sorafs_preview_publish.json`) deve ser definido como SoraFS.
   - يرفع الموقع الثابت, وآثار البيانات الوصفية (`docs-portal-preview`, `docs-portal-preview-metadata`), وحزمة SoraFS (`docs-portal-preview-sorafs`).
2. Use o CI para usar a soma de verificação do seu script GitHub (o script do GitHub está lá). `docs-portal-preview.yml`).
3. Verifique o valor de `docs/portal/README.md` (CI) e verifique o valor do arquivo no computador.

## سكربت التحقق

`docs/portal/scripts/preview_verify.sh` é uma ferramenta que pode ser usada para `sha256sum`. Use `npm run serve` (ou seja, o código de barras `npm run serve:verified`) para obter informações sobre `docusaurus serve`. واحدة عند مشاركة اللقطات المحلية. منطق التحقق:

1. Selecione o modelo SHA (`sha256sum` ou `shasum -a 256`) como `build/checksums.sha256`.
2. يقارن اختياريا digest/اسم ملف واصف المعاينة `checksums_manifest`, وعند توفره digest/اسم ملف أرشيف المعاينة.
3. Você pode fazer isso sem precisar de ajuda Não.

مثال الاستخدام (بعد استخراج آثار CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

يجب على مهندسي CI e والإصدارات تشغيل السكربت كلما حمّلوا حزمة معاينة, أو أرفقوا آثارا بتذكرة إصدار.

## Passo 2 — Nome SoraFS

1. وسّع سير عمل المعاينة بوظيفة تقوم بـ:
   - رفع الموقع المبني إلى بوابة staging em SoraFS باستخدام `sorafs_cli car pack` e `manifest submit`.
   - Resumo do resumo do código e CID para SoraFS.
   - Altere `{ commit, branch, checksum_manifest, cid }` para Norito JSON (`docs/portal/preview/preview_descriptor.json`).
2. Verifique o CID e o CID do seu computador.
3. Verifique se o `sorafs_cli` é executado a seco e funciona a seco. مع التغييرات المستقبلية.

## المرحلة 3 — الحوكمة والتدقيق

1. Verifique se o Norito (`PreviewDescriptorV1`) está conectado ao `docs/portal/schemas/`.
2. Verifique o documento DOCS-SORA:
   - Verifique `sorafs_cli manifest verify` do código CID.
   - Obtenha o resumo da soma de verificação e o CID do programa PR.
3. Verifique o valor da soma de verificação com a soma de verificação da sua conta.

## المخرجات والمسؤوليات

| المعلم | المالك | الهدف | Produtos |
|--------|--------|-------|---------|
| A soma de verificação é calculada no CI | Máquinas de lavar roupa | Número 1 | Não há problema em fazer isso. |
| Chave de fenda SoraFS | بنية تحتية للوثائق / فريق التخزين | Número 2 | O teste de teste é feito com Norito. |
| تكامل الحوكمة | Acesse Docs/DevRel / فريق عمل الحوكمة | Número 3 | ينشر المخطط ويحدّث قوائم التدقيق وبنود خارطة الطريق. |

## أسئلة مفتوحة

- Não há nenhum problema em SoraFS (staging أم مسار معاينة مخصص)؟
- هل نحتاج توقيعات مزدوجة (Ed25519 + ML-DSA) على واصف المعاينة قبل النشر؟
- Você pode usar o orquestrador CI تهيئة (`orchestrator_tuning.json`) com o `sorafs_cli` para usar إعادة إنتاج البيانات؟

Verifique o valor de `docs/portal/docs/reference/publishing-checklist.md` e verifique o valor do arquivo.