---
lang: ar
direction: rtl
source: docs/portal/README.md
status: complete
translator: manual
source_hash: 4b0d6c295c7188355e2c03d7c8240271da147095ff557fae2152f42e27bd17fa
source_last_modified: "2025-11-14T04:43:03.939564+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/portal/README.md (SORA Nexus Developer Portal) -->

# بوابة مطوري SORA Nexus

يستضيف هذا المجلد workspace الخاص بـ Docusaurus للبوابة التفاعلية للمطورين. تجمع
البوابة بين أدلة Norito و quickstarts الخاصة بـ SDK ومرجع OpenAPI الذي يولّده
`cargo xtask openapi`، وتعرضها كلها ضمن هوية SORA Nexus البصرية المستخدمة في برنامج
الوثائق بأكمله.

## المتطلبات المسبقة

- Node.js 18.18 أو أحدث (baseline لـ Docusaurus v3).
- Yarn 1.x أو npm ≥ 9 لإدارة الحزم.
- Toolchain خاص بـ Rust (يستخدمه سكربت مزامنة OpenAPI).

## Bootstrap

```bash
cd docs/portal
npm install    # أو yarn install
```

## الأوامر المتاحة

| الأمر | الوصف |
|-------|-------|
| `npm run start` / `yarn start` | تشغيل خادم تطوير محلي مع live reload (افتراضيًا `http://localhost:3000`). |
| `npm run build` / `yarn build` | إنشاء build للإنتاج في `build/`. |
| `npm run serve` / `yarn serve` | خدمة أحدث build محليًا (مفيد لـ smoke tests). |
| `npm run docs:version -- <label>` | أخذ snapshot من الوثائق الحالية إلى `versioned_docs/version-<label>` (غلاف حول `docusaurus docs:version`). |
| `npm run sync-openapi` / `yarn sync-openapi` | إعادة توليد `static/openapi/torii.json` عبر `cargo xtask openapi` (استخدم `--mirror=<label>` لنسخ الـ spec إلى snapshots أخرى). |
| `npm run tryit-proxy` | تشغيل بروكسي staging الذي يدعم وحدة تحكم “Try it” (انظر قسم الإعداد أدناه). |
| `npm run probe:tryit-proxy` | إرسال استعلام `/healthz` + طلب تجريبي ضد البروكسي (أداة مساعدة لـ CI/المراقبة). |
| `npm run manage:tryit-proxy -- <update|rollback>` | تحديث أو استرجاع الهدف `.env` الخاص بالبروكسي مع دعم نسخ backup. |
| `npm run sync-i18n` | التأكد من وجود stubs للترجمة لليابانية والعبرية والإسبانية والبرتغالية والفرنسية والروسية والعربية والأردية ضمن `i18n/`. |
| `npm run sync-norito-snippets` | إعادة توليد مستندات أمثلة Kotodama المنسّقة + snippets قابلة للتنزيل (كما يتم تشغيله تلقائيًا بواسطة إضافة خادم التطوير). |
| `npm run test:tryit-proxy` | تشغيل اختبارات الوحدة الخاصة بالبروكسي باستخدام test runner الخاص بـ Node (`node --test`). |

يتطلب سكربت مزامنة OpenAPI أن يكون `cargo xtask openapi` متاحًا من جذر المستودع؛ حيث
ينتج ملف JSON حتميًا في `static/openapi/` ويتوقع الآن أن يكشف router الخاص بـ Torii
عن spec حية (استخدم `cargo xtask openapi --allow-stub` فقط في حالات الطوارئ ولإخراج
placeholders مؤقتة).

## إصدار الوثائق ولقطات OpenAPI

- **إنشاء إصدار من الوثائق:** نفّذ `npm run docs:version -- 2025-q3` (أو أي label يتم
  الاتفاق عليه). نفّذ commit على `versioned_docs/version-<label>` و
  `versioned_sidebars` و `versions.json`. سيعرض شريط التنقل (navbar) تلقائيًا snapshot
  الجديد في قائمة الإصدارات.
- **مزامنة artefacts الخاصة بـ OpenAPI:** بعد إنشاء إصدار، حدّث spec الكانونية
  و manifest باستخدام `cargo xtask openapi --sign <مسار-مفتاح-ed25519>`، ثم التقط
  snapshot مطابقًا باستخدام
  `npm run sync-openapi -- --version=2025-q3 --mirror=current --latest`. يكتب السكربت
  الملف `static/openapi/versions/2025-q3/torii.json`، وينسخ الـ spec إلى
  `versions/current/torii.json`، ويحدّث `versions.json`، ويحدّث `/openapi/torii.json`،
  ويستنسخ `manifest.json` الموقّع إلى كل مجلد إصدار، بحيث تحمل الـ specs التاريخية
  نفس metadata الخاص بمنشأها. يمكنك تمرير أي عدد من `--mirror=<label>` لنسخ الـ spec
  الجديدة إلى Snapshots تاريخية إضافية.
- **توقّعات CI:** يجب أن تتضمن الـ commits التي تغيّر الوثائق، عند اللزوم، bump في
  رقم الإصدار و snapshots محدثة لـ OpenAPI حتى تتمكن واجهات Swagger وRapiDoc وRedoc
  من التبديل بين الـ specs التاريخية من دون أخطاء fetch.
- **فرض صحة manifest:** لا يقوم سكربت `sync-openapi` بنسخ manifests إلا إذا كانت نسخة
  `manifest.json` على القرص مطابقة للـ spec التي تم توليدها للتو. إذا تم تخطي النسخ،
  أعد تنفيذ `cargo xtask openapi --sign <مفتاح>` لتحديث الـ manifest الكانوني ثم
  أعد المزامنة حتى تلتقط snapshots النسخة الموقعة. يعيد
  `ci/check_openapi_spec.sh` تشغيل المولّد ويتحقق من manifest قبل السماح بدمج
  الـ commits.

## البنية

```text
docs/portal/
├── docs/                 # محتوى Markdown/MDX الخاص بالبوابة
├── i18n/                 # overrides خاصة باللغات (ja/he) يُنشئها sync-i18n
├── src/                  # صفحات/مكوّنات React (scaffolding)
├── static/               # أصول ثابتة تُقدَّم كما هي (تشمل JSON الخاص بـ OpenAPI)
├── scripts/              # سكربتات مساعدة (مزامنة OpenAPI)
├── docusaurus.config.js  # إعدادات الموقع الأساسية
└── sidebars.js           # نموذج شريط التنقل / sidebars
```

### إعداد بروكسي Try It

يمرّر Sandbox “Try it” الطلبات عبر `scripts/tryit-proxy.mjs`. اضبط البروكسي عبر
متغيرات بيئة قبل تشغيله:

```bash
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
```

في بيئات staging/الإنتاج، عرّف هذه المتغيرات ضمن نظام إعدادات المنصة (مثل secrets في
GitHub Actions أو متغيرات البيئة في orchestrator الحاويات).

### عناوين الـ preview وملاحظات الإصدارات

- نسخة preview عامة (beta): `https://docs.iroha.tech/`
- كما يعرض GitHub الـ build ضمن بيئة **github-pages** لكل عملية نشر.
- تحتوي طلبات الـ Pull التي تغيّر محتوى البوابة على artefacts من Actions
  (`docs-portal-preview`, `docs-portal-preview-metadata`) تشمل الموقع المبني و manifest
  للـ checksums والأرشيف المضغوط و descriptor؛ يمكن للمراجعين تنزيل `index.html` وفتحه
  محليًا والتحقق من الـ checksums قبل مشاركة روابط الـ preview. يضيف الـ workflow
  تعليقًا مختصرًا إلى كل PR (يتضمن hashes الخاصة بالـ manifest/الأرشيف وحالة SoraFS)
  لتوفير إشارة سريعة على نجاح عملية التحقق.
- استخدم
  `./docs/portal/scripts/preview_verify.sh --build-dir <مجلد build مفكوك> --descriptor <descriptor> --archive <archive>`
  بعد تنزيل bundle الـ preview للتأكد من أن artefacts تطابق ما أنتجته CI قبل مشاركة
  الرابط خارجيًا.
- عند إعداد ملاحظات الإصدارات أو تحديثات الحالة، استشهد بعنوان preview حتى يتمكن
  المراجعون الخارجيون من استعراض أحدث snapshot للبوابة من دون الحاجة إلى استنساخ
  المستودع.
- نسّق موجات الـ preview عبر
  `docs/portal/docs/devportal/preview-invite-flow.md` وبالتوازي مع
  `docs/portal/docs/devportal/reviewer-onboarding.md` بحيث تعيد كل دعوة و export خاص
  بالـ telemetry وخطوة offboarding استخدام نفس trail للأدلة.

</div>

