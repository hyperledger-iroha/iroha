---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#النسخة السابقة من اكسبوكس اير

DOCS-SORA هو عبارة عن ملف مطلوب وممدد للاشتراك وحزمة تم التحقق منها بواسطة المجموع الاختباري من خلال مسجلات Lukel للإصدار. سجل في بورنج (وادع المشاهدين) أكمل بعد ذلك دليل التشغيل الخاص بك باستخدام تقنية الدفع المسبق عبر الإنترنت.

## پيشگي قطاعي

- ريو على بورنغ ڤيو وجهة نظر و ريوريو ريوكر بدرج صغير فقط.
- أحدث إصدار من المنفذ `docs/portal/build/` موجود وتم فحص المجموع الاختباري (`build/checksums.sha256`).
- SoraFS إسناد أولي (Torii URL، السلطة، المفتاح الخاص، جمع الحقبة) يتم حفظ تحويلات إبلز أو تكوين JSON. [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- مطلوب اسم المضيف (`docs-preview.sora.link`, `docs.iroha.tech` وغيره) وهو عبارة عن DNS بديل وارتباط عند الطلب يشمل المزيد.

## رحلة 1 - حزمة البيانات والتحقق من کریں

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

تحقق من بيان المجموع الاختباري النصي الذي قد يكون موجودًا أو ما شابه قبل أن تقوم بإدراج إنكرتا، وقد تم تقديم المقالة السابقة.

## رحلةہ 2 - SoraFS القطع الأثرية پیك کریں

يمكن تغيير الموقع الذي يمثل السيارة الحتمية/البيان. `ARTIFACT_DIR` إلى ڈيفالويليو `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

يحتوي على `portal.car` و`portal.manifest.*` والواصف وبيان المجموع الاختباري للنسخة السابقة من لعبة منسل.

## مرحلہ 3 - پریویو الاسم المستعار شاع کریں

قم بإعادة استخدام دبوس المساعد الذي يستخدمه **بغير** `--skip-submit` لتكملة صفحة الايكسبوز الجديدة. علامات تكوين JSON أو CLI الصريحة:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

تم استخدام `portal.pin.report.json` و`portal.manifest.submit.summary.json` و`portal.submit.response.json`، مما يسمح لك بدعوة حزمة الأدلة التي ستساعدك أيضًا.

## خطوة 4 - خطة تغيير نظام أسماء النطاقات (DNS).

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

يحتوي الإصدار JSON على Ops على أحدث بيانات DNS التي تستبدل ملخص البيان هذا. إذا تم استخدام الواصف للتراجع عن الحالة السابقة، فهذا يعني أن `--previous-dns-plan path/to/previous.json` يشمل القراءة.

## الرحلة 5 - قم بالبحث عن مسبار

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

علامة إصدار مسبار جارى ورؤوس CSP وبيانات تعريف التوقيع التي يتم التحقق منها. يقوم كل من الريجينز بإعادة تشغيل المجلدات (أو حليقة إخراج منسلك) مع إضافة سكاكين أخرى إلى ذاكرة التخزين المؤقت الحافة.

##حزمة الأدلة

يتضمن الفيديو المسبق عناصر ظاهرية تتضمن الدعوة والدعوة إلى مليون حوالة:

| قطعة أثرية | قصدي |
|----------|------|
| `build/checksums.sha256` | تحتوي الحزمة الثابتة الثابتة على حزمة CI التي يبلغ عددها مليونًا. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | الحمولة الأساسية SoraFS + البيان. |
| `portal.pin.report.json`، `portal.manifest.submit.summary.json`، `portal.submit.response.json` | تقديم واضح وملزمة الاسم المستعار للجامعة. |
| `artifacts/sorafs/portal.dns-cutover.json` | البيانات التعريفية لنظام أسماء النطاقات (الرابط، الويب)، الترويج للمسار (`Sora-Route-Binding`)، خلاصة، مؤشر `route_plan` (مخطط JSON + قوالب الرأس)، معلومات تطهير ذاكرة التخزين المؤقت، وعمليات التراجع. |
| `artifacts/sorafs/preview-descriptor.json` | يحتوي الجهاز على واصف للأرشيف + المجموع الاختباري للملف. |
| إخراج `probe` | تم إصدار علامة الإصدار المتوقعة مؤخرًا من جديد. |