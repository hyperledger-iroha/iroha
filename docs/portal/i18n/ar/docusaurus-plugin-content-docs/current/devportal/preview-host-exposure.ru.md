---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تصميم معاينة العرض

تحتاج البطاقة الجديدة DOCS-SORA إلى استخدام المعاينة العامة في كل حزمة، والمجموع الاختباري المحقق، والمراجعين تحقق محليا. استخدم دليل التشغيل هذا بعد الانتهاء من إلحاق الخوادم (والموافقة على العرض)، لبدء تشغيل مضيف معاينة بيتا في المجموعة.

## الرغبة المسبقة

- حجوزات الانضمام الشاملة متوافقة ومثبتة في متتبع المعاينة.
- تم إدخال البوابة التالية إلى `docs/portal/build/` والمجموع الاختباري (`build/checksums.sha256`).
- معاينة بيانات SoraFS (Torii URL، السلطة، المفتاح الخاص، عصر التحكم) المخزنة في الحماية المؤقتة أو تكوين JSON، على سبيل المثال [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- فتح تذكرة تغيير DNS باستخدام اسم المضيف الجيد (`docs-preview.sora.link`، `docs.iroha.tech` وما إلى ذلك) وجهات الاتصال عند الطلب.

## الجزء 1 - اكتشف الحزمة وتحقق منها

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

يجب أن يستمر محقق البرنامج النصي في حالة إظهار المجموع الاختباري للإجابة أو الإضافة إلى أن الشركة المساهمة تدقق في جميع عناصر المعاينة.

## الجزء 2 - تعبئة القطع الأثرية SoraFS

قم بتصفح الموقع الإحصائي في تحديد سعر السيارة/البيان. `ARTIFACT_DIR` من خلال `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

قم بقص `portal.car`، `portal.manifest.*`، الواصف والمجموع الاختباري للبيان في موجة معاينة التذكرة.

## الجزء 3 - نشر الاسم المستعار للمعاينة

قم بتثبيت مساعد الدبوس **بدون** `--skip-submit`، عندما تتمكن من فتح المضيف. قم بالإعداد المسبق لتكوين JSON أو أعلام CLI الجديدة:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

أرسل الأمر `portal.pin.report.json` و`portal.manifest.submit.summary.json` و`portal.submit.response.json`، والتي يتم تضمينها في حزمة الأدلة.

## الجزء 4 - تصميم خطة قطع DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

يمكنك استخدام JSON عالي الجودة مع Ops لربط حجب DNS ببيان الملخص الدقيق. عند الاستخدام اللاحق للواصف السابق، ستتم إضافة `--previous-dns-plan path/to/previous.json`.

## الجزء 5 - التحقق من المضيف المتنوع

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

قم بالتحقق من علامة الإصدار المكتملة وتأمينات CSP والملاحظات التحويلية. قم بإرشاد الأمر من منطقتين (أو قم بتشغيل Curl)، ليرى المدققون أن ذاكرة التخزين المؤقت للحافة تتقدم.

##حزمة الأدلة

قم بتضمين العناصر التالية في موجة معاينة التذكرة وإضافتها في رسائل البريد الإلكتروني:

| قطعة أثرية | الاسم |
|----------|-----------|
| `build/checksums.sha256` | يوضح أن الحزمة تدعم بناء CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Канонический SoraFS الحمولة + البيان. |
| `portal.pin.report.json`، `portal.manifest.submit.summary.json`، `portal.submit.response.json` | يُظهر أن تنفيذ البيان والاسم المستعار الخاص بالموافقة كان ناجحًا. |
| `artifacts/sorafs/portal.dns-cutover.json` | تحويلات DNS (التذكرة، النقر، جهات الاتصال)، جدول إنتاج البيانات (`Sora-Route-Binding`)، تحديد `route_plan` (خطة JSON + رأس الشبكة)، نشرة تطهير ذاكرة التخزين المؤقت وتعليمات التراجع لـ Ops. |
| `artifacts/sorafs/preview-descriptor.json` | одписанный واصف، أرشيف خاص + المجموع الاختباري. |
| فيوفود `probe` | تأكد من أن المضيف المباشر ينشر علامة الإصدار المميزة. |