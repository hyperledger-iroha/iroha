---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل العرض الخاص بمضيف المعاينة

تتطلب خريطة الطريق DOCS-SORA أن تقوم جميع المعاينة العامة باستخدام نفس الحزمة التي تم التحقق منها من خلال المجموع الاختباري الذي تقوم به المراجعون محليًا. استخدم دليل التشغيل هذا بعد إعداد المراجعين (وتذكرة الموافقة على الدعوة) وقم بتشغيله بشكل كامل لاستضافة النسخة التجريبية عبر الإنترنت.

## المتطلبات المسبقة

- عند إلحاق المراجعين بالموافقة والتسجيل بدون تتبع المعاينة.
- تم إنشاء البوابة الأخيرة في `docs/portal/build/` وتم التحقق من المجموع الاختباري (`build/checksums.sha256`).
- اعتماد معاينة SoraFS (URL Torii، autoridade، الخصوصية، العصر المرسل) مخزّن في بيئة متغيرة أو في تكوين JSON مثل [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- تذكرة فتح نظام أسماء النطاقات عبر اسم المضيف المطلوب (`docs-preview.sora.link`، `docs.iroha.tech`، وما إلى ذلك) بالإضافة إلى جهات الاتصال عند الطلب.

## الخطوة 1 - إنشاء الحزمة والتحقق منها

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

يتم استعادة نص التحقق عندما يكون بيان المجموع الاختباري صحيحًا أو مغشوشًا، مع الحفاظ على كل نسخة مصطنعة من المعاينة التي تم تدقيقها.

## باسو 2 - Empacotar os artefatos SoraFS

قم بتحويل موقع الويب إلى مستوى CAR / الحتمية الواضحة. `ARTIFACT_DIR` و `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

الملحق `portal.car`، `portal.manifest.*`، أو الواصف وبيان المجموع الاختباري في تذكرة المعاينة.

## الخطوة 3 - نشر الاسم المستعار للمعاينة

أعد تنفيذ مساعد الدبوس **sem** `--skip-submit` عندما يتم إعداده قريبًا لتصدير المضيف. Forneca o config JSON أو علامات CLI الصريحة:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

o الأمر الجرافيكي `portal.pin.report.json` و`portal.manifest.submit.summary.json` و`portal.submit.response.json`، الذي يجب أن يرافقه حزمة أدلة الدعوة.

## Passo 4 - تغيير مسار DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

قم بمشاركة نتائج JSON مع العمليات من أجل تعديل مرجع DNS أو ملخص البيان. من أجل إعادة استخدام الواصف السابق كأصل التراجع، قم بإضافة `--previous-dns-plan path/to/previous.json`.

## الخطوة 5 - اختبار المضيف المزروع

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

يؤكد المسبار علامة إصدار الخادم ورؤوس CSP وامتدادات الاغتيال. كرر الأمر من منطقتين (أو قم بإرفاق كلمة تجعيد) حتى يتمكن المستمعون من معرفة ما إذا كانت ذاكرة التخزين المؤقت لهذه الحافة قد وصلت إلى هذا الحد.

## حزمة الأدلة

قم بتضمين الإرشادات التالية على تذكرة عند المعاينة والمراجع عبر البريد الإلكتروني للدعوة:

| ارتيفاتو | اقتراح |
|----------|----------|
| `build/checksums.sha256` | أثبت أن الحزمة تتوافق مع بناء CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | الحمولة النافعة canonico SoraFS + البيان. |
| `portal.pin.report.json`، `portal.manifest.submit.summary.json`، `portal.submit.response.json` | أظهر أنك ترسل البيان + الاسم المستعار الملزم للمنتدى الختامي. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadados DNS (التذكرة، والرسائل، وجهات الاتصال)، واستئناف العرض الترويجي (`Sora-Route-Binding`)، وPonteiro `route_plan` (مخطط JSON + قوالب الرأس)، ومعلومات مسح ذاكرة التخزين المؤقت وتعليمات التراجع لـ Ops. |
| `artifacts/sorafs/preview-descriptor.json` | لقد تم تنفيذ الواصف الذي يربط الأرشيف + المجموع الاختباري. |
| صيدا دو `probe` | قم بتأكيد أن المضيف في الحياة سيعلن عن علامة الإصدار المنتظرة. |