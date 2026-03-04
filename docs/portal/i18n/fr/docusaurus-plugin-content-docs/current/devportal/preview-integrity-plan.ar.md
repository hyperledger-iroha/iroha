---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# خطة المعاينة المحكومة بالتحقق من checksum

توضح هذه الخطة العمل المتبقي المطلوب لجعل كل أثر معاينة في البوابة قابلا للتحقق قبل النشر. Il s'agit d'une somme de contrôle et d'une somme de contrôle L'installation est basée sur SoraFS et Norito.

## الأهداف

- **عمليات بناء حتمية:** تأكد من أن `npm run build` ينتج مخرجات قابلة لإعادة الإنتاج ويصدر دائما `build/checksums.sha256`.
- **معاينات متحقق منها :** اشترط أن يصاحب كل أثر معاينة بيان checksum وارفض النشر عندما يفشل التحقق.
- **Les frais de validation sont liés à Norito :** Les frais de validation (pour le commit de la somme de contrôle digest et le CID) SoraFS) ou Norito JSON pour les fichiers à valeur ajoutée.
- **أدوات للمشغلين:** وفر سكربت تحقق بخطوة واحدة يمكن للمستهلكين تشغيله محليا (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); يقوم السكربت الآن بتغليف تدفق التحقق من checksum + الواصف من البداية إلى النهاية. أمر المعاينة القياسي (`npm run serve`) يستدعي هذا المساعد تلقائيا قبل `docusaurus serve` كي تبقى اللقطات المحلية Utilisez la somme de contrôle (pour `npm run serve:verified`, utilisez la somme de contrôle).

## المرحلة 1 — فرض CI1. Utilisez `.github/workflows/docs-portal-preview.yml` pour :
   - يشغل `node docs/portal/scripts/write-checksums.mjs` pour Docusaurus (مستدعى محليا بالفعل).
   - ينفذ `cd build && sha256sum -c checksums.sha256` ويفشل المهمة عند عدم التطابق.
   - Vous pouvez construire avec `artifacts/preview-site.tar.gz` et avec la somme de contrôle et avec `scripts/generate-preview-descriptor.mjs` et avec `scripts/sorafs-package-preview.sh`. JSON (`docs/examples/sorafs_preview_publish.json`) est également compatible avec les fichiers SoraFS.
   - يرفع الموقع الثابت، وآثار البيانات الوصفية (`docs-portal-preview`, `docs-portal-preview-metadata`) et SoraFS (`docs-portal-preview-sorafs`) حتى يمكن فحص البيان وملخص CAR والخطة بدون إعادة البناء.
2. Si vous utilisez CI pour utiliser la somme de contrôle avec la somme de contrôle (en utilisant le script GitHub pour `docs-portal-preview.yml`).
3. Utilisez le `docs/portal/README.md` (قسم CI) et les informations relatives à votre appareil.

## سكربت التحقق

`docs/portal/scripts/preview_verify.sh` يتحقق من آثار المعاينة التي تم تنزيلها دون الحاجة لاستدعاءات يدوية لـ `sha256sum`. `npm run serve` (pour la prise en charge `npm run serve:verified`) pour les applications `docusaurus serve`. واحدة عند مشاركة اللقطات المحلية. منطق التحقق:

1. Utilisez le module SHA (`sha256sum` et `shasum -a 256`) pour `build/checksums.sha256`.
2. يقارن اختياريا digest/اسم ملف واصف المعاينة `checksums_manifest`, et وعند توفره digest/اسم ملف أرشيف المعاينة.
3. ينهي بخروج غير صفري عند اكتشاف أي عدم تطابق حتى يتمكن المراجعون من حظر معاينات تم العبث بها.

مثال الاستخدام (بعد استخراج آثار CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```يجب على مهندسي CI والإصدارات تشغيل السكربت كلما حمّلوا حزمة معاينة أو أرفقوا آثارا بتذكرة إصدار.

## المرحلة 2 — نشر SoraFS

1. وسّع سير عمل المعاينة بوظيفة تقوم بـ:
   - رفع الموقع المبني إلى بوابة staging for SoraFS by `sorafs_cli car pack` et `manifest submit`.
   - التقاط digest البيان المعاد ومعرف CID لـ SoraFS.
   - Remplacer `{ commit, branch, checksum_manifest, cid }` par Norito JSON (`docs/portal/preview/preview_descriptor.json`).
2. Mettez en place le CID et le CID.
3. Utilisez le mode `sorafs_cli` pour le fonctionnement à sec en utilisant la fonction de fonctionnement à sec. التغييرات المستقبلية.

## المرحلة 3 — الحوكمة والتدقيق

1. Utilisez Norito (`PreviewDescriptorV1`) pour utiliser `docs/portal/schemas/`.
2. Utilisez le document DOCS-SORA pour :
   - تشغيل `sorafs_cli manifest verify` pour CID المرفوع.
   - تسجيل digest parيان checksum و CID في وصف PR الإصدار.
3. Utilisez la somme de contrôle pour utiliser la somme de contrôle.

## المخرجات والمسؤوليات| المعلم | المالك | الهدف | ملاحظات |
|--------|--------|-------|---------|
| تطبيق تحقق checksum pour CI | بنية تحتية للوثائق | الأسبوع 1 | يضيف بوابة فشل ورفع آثار. |
| نشر معاينات SoraFS | بنية تحتية للوثائق / فريق التخزين | الأسبوع 2 | يتطلب الوصول لاعتمادات staging وتحديثات مخطط Norito. |
| تكامل الحوكمة | قائد Docs/DevRel / فريق عمل الحوكمة | الأسبوع 3 | ينشر المخطط ويحدّث قوائم التدقيق وبنود خارطة الطريق. |

## أسئلة مفتوحة

- أي بيئة في SoraFS يجب أن تستضيف آثار المعاينة (staging أم مسار معاينة مخصص)؟
- هل نحتاج توقيعات مزدوجة (Ed25519 + ML-DSA) على واصف المعاينة قبل النشر؟
- هل يجب أن يثبّت سير عمل CI تهيئة orchestrator (`orchestrator_tuning.json`) عند تشغيل `sorafs_cli` للحفاظ على قابلية إعادة إنتاج البيانات؟

وثّق القرارات في `docs/portal/docs/reference/publishing-checklist.md` وحدث هذه الخطة عند حسم المجهولات.