---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## فيساو جيرال

يقوم هذا قواعد اللعبة بتحويل عناصر نظام التشغيل إلى خريطة الطريق **DOCS-7** (النشرة العامة لـ SoraFS) و **DOCS-8**
(تلقائيًا على دبوس CI/CD) في إجراء عملي لبوابة المطورين.
قم بقطع عملية البناء/النسيج، أو تغليف SoraFS، وتركيب البيانات مع Sigstore،
الترويج للاسم المستعار والتحقق وتدريبات التراجع حتى تتمكن من معاينة كل إصدار وإصداره
سيتم إعادة إنتاجها وتدقيقها.

التدفق يفترض أنك تتحدث عن الثنائي `sorafs_cli` (المنشأ مع `--features cli`)، الوصول إلى
نقطة النهاية Torii مع أذونات تسجيل الدبوس، وبيانات الاعتماد OIDC لـ Sigstore. حراسة العزلة
de longa duracao (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, الرموز المميزة لـ Torii) في قبو CI; كما
يمكن للتنفيذيين المحليين نقلهم من عمليات التصدير.

## المتطلبات المسبقة

- العقدة 18.18+ com `npm` أو `pnpm`.
- `sorafs_cli` من `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- عنوان URL الخاص بـ Torii الذي يعرض `/v1/sorafs/*` لديه حساب/لديه خصوصية التفويض الذي يمكنه إرسال البيانات والأسماء المستعارة.
- الباعث OIDC (GitHub Actions، GitLab، هوية عبء العمل، وما إلى ذلك) لإصدار `SIGSTORE_ID_TOKEN`.
- اختياري: `examples/sorafs_cli_quickstart.sh` للتشغيل و`docs/source/sorafs_ci_templates.md` لدعم سير العمل في GitHub/GitLab.
- تكوين كمتغيرات OAuth de Tre it (`DOCS_OAUTH_*`) وتنفيذ
 [قائمة التحقق من تعزيز الأمان](./security-hardening.md) قبل ترقية الإصدار
 مختبر النار. قم ببناء البوابة الآن عندما تكون متغيرات faltan
 عندما يتم استخدام مقابض TTL/الاقتراع؛ importa
 `DOCS_OAUTH_ALLOW_INSECURE=1` مفتوح لمعاينة الإعدادات المحلية القابلة للإلغاء. الملحق أ
 دليل اختبار القلم على تذكرة الإصدار.

## Etapa 0 - التقط حزمة وكيل

قبل الترويج لمعاينة Netlife أو البوابة، يتم بيعها كمصادر للوكيل.
ملخص البيان OpenAPI الثابت في حزمة محددة:

```bash
cd docs/portal
npm run release:tryit-proxe -- \
 --out ../../artifacts/tryit-proxy/$(date -u +%E%m%dT%H%M%SZ) \
 --target https://torii.dev.sora \
 --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` نسخ مساعدي الوكيل/التحقيق/الاستعادة، والتحقق من الاغتيال
OpenAPI وأكتب `release.json` mas `checksums.sha256`. Anexe este pacote al Ticket de
بوابة Netlify/SoraFS الترويجية حتى تتمكن المراجعات من إعادة إنتاجها بالشكل الدقيق
يتم إعادة بناء الوكيل كنقاط للهدف Torii. O paquete Tambem Registra si os
مقدمو الدعم من قبل العملاء المؤهلين (`allow_client_auth`) لدعم الخطة
يتم بدء التشغيل وإعادة ضبط CSP من خلال المزامنة.

## Etapa 1 - بناء شبكة البوابة

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` ينفذ `scripts/write-checksums.mjs` تلقائيًا، وينتج:

- `build/checksums.sha256` - البيان SHA256 المناسب لـ `sha256sum -c`.
- `build/release.json` - البيانات الوصفية (`tag`، `generated_at`، `source`) موجودة في كل CAR/manifesto.

احصل على المزيد من المحفوظات بجانب السيرة الذاتية CAR حتى يتمكن المراجعون من مقارنة المصنوعات اليدوية
معاينة إعادة بناء sem.

## إتابا 2 - Empaqueta os الأصول estaticos

قم بتنفيذ عملية تعبئة CAR مقابل إخراج دليل Docusaurus. كمثال على ذلك
اكتب جميع الأعمال الفنية bajo `artifacts/devportal/`.

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
 --input build \
 --car-out "$OUT"/portal.car \
 --plan-out "$OUT"/portal.plan.json \
 --summary-out "$OUT"/portal.car.json \
 --chunker-handle sorafs.sf1@1.0.0
```

يلتقط JSON محتوى القطع والخلاصات ومسارات التخطيط لإثبات ذلك
`manifest build` ويتم إعادة استخدام لوحات معلومات CI بعد ذلك.

## Etapa 2b - رفاق Empaqueta OpenAPI e SBOM

يتطلب DOCS-7 نشر موقع البوابة أو لقطة OpenAPI وحمولات نظام التشغيل SBOM
مثل البيانات المميزة التي تمكن البوابات من التقاط الرؤوس
`Sora-Proof`/`Sora-Content-CID` لكل قطعة أثرية. يا مساعد دي الافراج
(`scripts/sorafs-pin-release.sh`) موجود في المجلد OpenAPI
(`static/openapi/`) و SBOMs الصادرة عبر `syft` في السيارات المنفصلة
`openapi.*`/`*-sbom.*` وتسجيل البيانات التعريفية
`artifacts/sorafs/portal.additional_assets.json`. دليل التنفيذ أو الجريان،
كرر الخطوات 2-4 لكل حمولة مع تفضيلاتك الخاصة وعلامات التعريف
(على سبيل المثال `--car-out "$OUT"/openapi.car` ماس
`--metadata alias_label=docs.sora.link/openapi`). سجل كل بيان/اسم مستعار
في Torii (الموقع، OpenAPI، SBOM في البوابة، SBOM في OpenAPI) قبل تغيير DNS لذلك
يمكن للبوابة تقديم خدمة تجريبية لجميع العناصر المنشورة.

## إيتابا 3 – بناء البيان

```bash
sorafs_cli manifest build \
 --summare "$OUT"/portal.car.json \
 --manifest-out "$OUT"/portal.manifest.to \
 --manifest-json-out "$OUT"/portal.manifest.json \
 --pin-min-replicas 5 \
 --pin-storage-class warm \
 --pin-retention-epoch 14 \
 --metadata alias_label=docs.sora.link
```

ضبط الأعلام السياسية من خلال فتح نافذة التحرير (على سبيل المثال، `--pin-storage-class
hot` الفقرة الكناري). يعد خيار JSON اختياريًا ولكنه مناسب لمراجعة الكود.

## ايتابا 4 - Assinatura com Sigstore

```bash
sorafs_cli manifest sign \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --bundle-out "$OUT"/portal.manifest.bundle.json \
 --signature-out "$OUT"/portal.manifest.sig \
 --identity-token-provider github-actions \
 --identity-token-audience sorafs-devportal
```

سجل الحزمة أو ملخص البيان أو ملخصات القطع أو تجزئة الرمز المميز BLAKE3
OIDC غير مستمر أو JWT. حراسة لمدة طويلة أو حزمة كهجوم منفصل؛ كما الترقيات دي
يمكن للإنتاج إعادة استخدام كل المصنوعات اليدوية في مكان إعادة تجميعها. كما execucoes locais
يمكنك استبدال علامات موفر الخدمة com `--identity-token-env` (أو المُنشئ
`SIGSTORE_ID_TOKEN` في الداخل) عندما يقوم المساعد OIDC بإصدار رمز مميز خارجيًا.

## Etapa 5 - سجل الحسد

قم بإرسال البيان الثابت (خطة القطع) إلى Torii. اطلب دائمًا ملخصًا لذلك
مدخل/الاسم المستعار نتيجة سيجا Auditavel.

```bash
sorafs_cli manifest submit \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --torii-url "$TORII_URL" \
 --authorite ih58... \
 --private-kee "$IROHA_PRIVATE_KEY" \
 --submitted-epoch 20260101 \
 --alias-namespace docs \
 --alias-name sora.link \
 --alias-proof "$OUT"/docs.alias.proof \
 --summary-out "$OUT"/portal.submit.json \
 --response-out "$OUT"/portal.submit.response.json
```

عند إرسال اسم مستعار لمعاينة الصورة (`docs-preview.sora`)، كرر ذلك
أرسله باسم مستعار فريد حتى يتمكن ضمان الجودة من التحقق من المحتوى قبل الترويج له
إنتاج.

يتطلب الربط باستخدام الاسم المستعار ثلاثة مجالات: `--alias-namespace` و`--alias-name` و`--alias-proof`.
تنتج الحوكمة حزمة إثبات (base64 أو بايت Norito) عند تقديم الطلب
الاسم المستعار؛ قم بالحماية من عزلات CI وعرضها كملف قبل استدعاء `manifest submit`.
قم بوضع علامات الأسماء المستعارة التي لم يتم تثبيتها عند فتح ملف أو بيان بدون استخدام DNS.

## الجزء 5 ب - نوع من أنواع الحكم

يجب على كل بيان أن يسافر مع قائمة مقترحة للبرلمان من أجل أي مدينة
يمكنك تقديم أي تغيير دون الحصول على امتيازات الاعتماد. Depois de os pasos
إرسال/توقيع، تنفيذ:

```bash
sorafs_cli manifest proposal \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --submitted-epoch 20260101 \
 --alias-hint docs.sora.link \
 --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` يلتقط تعليمات canonica `RegisterPinManifest`,
o ملخص القطع والسياسة ومسار الاسم المستعار. مساعد على تذكرة الحكم أو الجميع
بوابة البرلمان حتى يتمكن المفوضون من مقارنة الحمولة دون إعادة بناء المصنوعات اليدوية.
كما لو كان الأمر لا يضغط على المفتاح الرئيسي لـ Torii، يمكن لأي مدينة تعديلها
propuest localmente.

## الخطوة 6 - التحقق من البيانات والقياس عن بعد

بعد البدء، قم بتنفيذ خطوات التحقق المحددة:

```bash
sorafs_cli proof verife \
 --manifest "$OUT"/portal.manifest.to \
 --car "$OUT"/portal.car \
 --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
 --manifest "$OUT"/portal.manifest.to \
 --bundle "$OUT"/portal.manifest.bundle.json \
 --chunk-plan "$OUT"/portal.plan.json
```

- التحقق `torii_sorafs_gateway_refusals_total` ه
 `torii_sorafs_replication_sla_total{outcome="missed"}` للشذوذ.
- قم بتنفيذ `npm run probe:portal` لتشغيل وكيل Try-It والروابط المسجلة
 ضد المحتوى المستلم.
- التقاط أدلة المراقبة الموصوفة
 [النشر والمراقبة](./publishing-monitoring.md) لبوابة المراقبة DOCS-3c
 إنه مرضي إلى جانب خطوات النشر. يا مساعد ahora acepta مضاعفات الانتراداس
 `bindings` (الموقع، OpenAPI، SBOM للبوابة، SBOM في OpenAPI) ويتم تطبيق `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
 قم باستضافة الهدف عبر حارس اختياري `hostname`. A invocacao de abajo يكتب tanto um
 ملخص JSON موحد كحزمة الأدلة (`portal.json`، `tryit.json`، `binding.json` e
 `checksums.sha256`) خلف دليل الإصدار:

 ```bash
 npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
 --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
 ```

## Etapa 6a - مخطط البوابة المعتمد

اشتقاق خطة SAN/تحدي TLS قبل إنشاء حزم GAR لمعدات البوابة ونظام التشغيل
يقوم مطورو DNS بمراجعة بعض الأدلة. يعكس المساعد الجديد تكاليف السيارة
يستضيف عدد DG-3 رموز البدل الأساسية وشبكات SAN المضيفة الجميلة وميزات DNS-01 والتحديات التي توصي بها ACME:

```bash
cargo xtask soradns-acme-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.acme-plan.json
```

قم بإدراج JSON جنبًا إلى جنب مع حزمة الإصدار (أو تحتها مع تذكرة التغيير) للمشغلين
يمكنك ربط قيم SAN بتكوين `torii.sorafs_gateway.acme` de Torii ومراجعتها
يمكن لـ GAR تأكيد الخرائط الكنسي/جميلة دون إعادة تنفيذ اشتقاقات المضيف. أجريجا
الوسيطات `--name` إضافية لكل عرض ترويجي بسيط في نفس الإصدار.

## Etapa 6b - مشتق من خرائط المضيف الكنسي

قبل تحميل حمولة الهيكل GAR، قم بالتسجيل أو تحديد خريطة المضيف لكل اسم مستعار.
`cargo xtask soradns-hosts` hashea cada `--name` على علامتك Canonica
(`<base32>.gw.sora.id`)، قم بإصدار حرف بدل مطلوب (`*.gw.sora.id`) واشتقاق مضيف جميل
(`<alias>.gw.sora.name`). استمر في إصدار القطع الأثرية حتى تتم مراجعتها
DG-3 يمكن مقارنتها بالخريطة بجوار GAR:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json
```

الولايات المتحدة الأمريكية `--verify-host-patterns <file>` للسقوط السريع عند استخدام JSON de GAR أو ربط البوابة
حذف أحد المضيفين المطلوبين. يا مساعد قبول عدة أرشيفات التحقق، هاسيندو
من السهل زراعة الوبر مثل `portal.gateway.binding.json` في نفس الاستدعاء:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json \
 --verify-host-patterns artifacts/sorafs/portal.gar.json \
 --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

قم بإرفاق ملخص JSON وسجل التحقق من بطاقة تغيير DNS/البوابة لذلك
يمكن للمراجعين تأكيد المضيفين الكنسيين وأحرف البدل والجميل دون إعادة تنفيذ البرامج النصية.
أعد تنفيذ الأمر عند إضافة أسماء مستعارة جديدة إلى الحزمة لتحديث GAR
هنا دليل واضح.

## Etapa 7 - نوع واصف DNS المقطوعتتطلب عمليات قطع الإنتاج حزمة من التغييرات التدقيقية. بعد تقديم الخروج
(الاسم المستعار ملزم)، أو انبعاث المساعد
`artifacts/sorafs/portal.dns-cutover.json`، الالتقاط:

- بيانات التعريف المرتبطة بالاسم المستعار (مساحة الاسم/الاسم/الإثبات، ملخص البيان، URL Torii،
 عصر الإرسال، autoridad)؛
- سياق الإصدار (العلامة، التسمية المستعارة، مسارات البيان/CAR، خطة القطع، الحزمة Sigstore)؛
- نقاط التحقق (مسبار الأمر، الاسم المستعار + نقطة النهاية Torii)؛ ه
- المجالات الاختيارية للتحكم في التغيير (معرف التذكرة، نافذة القطع، عمليات الاتصال،
 اسم المضيف/منطقة الإنتاج)؛
- بيانات التعريف الترويجية للمسارات المشتقة من الرأس `Sora-Route-Binding`
 (المضيف Canonico/CID، مسارات الرأس + الربط، أوامر التحقق)، ضمان الترويج
 تشير التدريبات الاحتياطية GAR إلى بعض الأدلة؛
- المصنوعات اليدوية لخطة الطريق التي تم إنشاؤها (`gateway.route_plan.json`،
 قوالب الرؤوس ورؤوس التراجع الاختيارية) لتذاكر التغيير والخطافات
 يمكن لخيط CI التحقق من أن كل حزمة DG-3 تشير إلى خطط الترويج/التراجع
 canonicos antes de aprobacion؛
- البيانات التعريفية الاختيارية لإبطال ذاكرة التخزين المؤقت (نقطة نهاية التطهير، متغير المصادقة، الحمولة JSON،
 ومثال على الأمر `curl`)؛ ه
- تلميحات التراجع عن الواصف السابق (علامة الإصدار وملخص البيان)
 لكي يتم التقاط التذاكر بطريقة احتياطية محددة.

عندما يتطلب الإصدار تنظيف ذاكرة التخزين المؤقت، قم بإنشاء خطة أساسية جنبًا إلى جنب مع واصف القطع:

```bash
cargo xtask soradns-cache-plan \
 --name docs.sora \
 --path / \
 --path /gateway/manifest.json \
 --auth-header Authorization \
 --auth-env CACHE_PURGE_TOKEN \
 --json-out artifacts/sorafs/portal.cache_plan.json
```

الملحق أو `portal.cache_plan.json` الناتج عن الحزمة DG-3 للمشغلين
يتم تحديد المضيفين/المسارات (وتلميحات المصادقة التي تتزامن) عن طريق إرسال طلبات `PURGE`.
يمكن اختيار ذاكرة التخزين المؤقت للواصف الرجوع إلى هذا الأرشيف مباشرة،
الحفاظ على مراجعات التحكم في التغيير بشكل دقيق حول كيفية تنظيف نقاط النهاية
خلال فترة انقطاع.

تتطلب كل حزمة DG-3 أيضًا قائمة مرجعية للترويج + التراجع. جنرالا عبر
`cargo xtask soradns-route-plan` حتى تتمكن مراجعات التحكم في التغيير من متابعة الخطوات
دقة الاختبار المبدئي والانتقال إلى الحالة السابقة من خلال الاسم المستعار:

```bash
cargo xtask soradns-route-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/gateway.route_plan.json
```

يستضيف O `gateway.route_plan.json` emitido captura Canonicos/Pretty، مسجلات التحقق من الصحة
من خلال الخطوات وتحديثات ربط GAR وإزالة ذاكرة التخزين المؤقت وإجراءات التراجع. بما في ذلك كوم
هذه المصنوعات GAR/الربط/القطع قبل إرسال أو تذكرة التغيير حتى تتمكن العمليات من تجربتها
aprobar os mesmos etapas com guion.

`scripts/generate-dns-cutover-plan.mjs` الدافع هو هذا الواصف ويتم تنفيذه تلقائيًا من هنا
`sorafs-pin-release.sh`. لتجديد أو تخصيص يدويا:

```bash
node scripts/generate-dns-cutover-plan.mjs \
 --pin-report artifacts/sorafs/portal.pin.report.json \
 --out artifacts/sorafs/portal.dns-cutover.json \
 --change-ticket OPS-4821 \
 --dns-hostname docs.sora.link \
 --dns-zone sora.link \
 --ops-contact docs-oncall@sora.link \
 --cache-purge-endpoint https://cache.api/purge \
 --cache-purge-auth-env CACHE_PURGE_TOKEN \
 --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

قم باستعادة البيانات التعريفية الاختيارية عبر متغيرات الإعداد قبل التنفيذ أو مساعد التثبيت:

| متغير | اقتراح |
| --- | --- |
| `DNS_CHANGE_TICKET` | معرف التذكرة مخزن في الواصف. |
| `DNS_CUTOVER_WINDOW` | فتحة التهوية ISO8601 (على سبيل المثال، `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`، `DNS_ZONE` | اسم المضيف للإنتاج + منطقة التشغيل التلقائية. |
| `DNS_OPS_CONTACT` | الاسم المستعار عند الطلب أو الاتصال بالتصعيد. |
| `DNS_CACHE_PURGE_ENDPOINT` | نقطة نهاية مسح ذاكرة التخزين المؤقت المسجلة في الواصف. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var que contiene o token de purge (الافتراضي: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | استخدم واصف القطع السابق لبيانات التعريف الخاصة بالتراجع. |

ملحق أو JSON لمراجعة تغيير DNS حتى يتمكن الموردون من التحقق من خلاصات البيان،
روابط الأسماء المستعارة والأوامر تستكشف نفسها لمراجعة سجلات CI. أعلام CLI
`--dns-change-ticket`، `--dns-cutover-window`، `--dns-hostname`،
`--dns-zone`، `--ops-contact`، `--cache-purge-endpoint`،
`--cache-purge-auth-env`، e `--previous-dns-plan` يثبت تجاوزات نظام التشغيل
عندما يتم تنفيذ أو تشغيل مساعد CI.

## الخطوة 8 - إنشاء ملف منطقة الحل (اختياري)

عندما يتم التعرف على نافذة قطع الإنتاج، يمكن أن يصدر نص الإصدار
قم بإنشاء ملف منطقة SNS ومقتطف من الحل تلقائيًا. قم بتثبيت سجلات DNS المطلوبة
البيانات التعريفية عبر متغيرات المحتوى أو خيارات سطر الأوامر؛ يا مساعد اللامارا أ
`scripts/sns_zonefile_skeleton.py` فورًا بعد إنشاء أو وصف القطع.
أثبت الحد الأدنى من قيمة A/AAAA/CNAME وهضم GAR (BLAKE3-256 من حمولة GAR الثابتة). سي أ
المنطقة/اسم المضيف هي conocidos e `--dns-zonefile-out` إذا حذفتها، أو ساعد في كتابتها
`artifacts/sns/zonefiles/<zone>/<hostname>.json` ولدينا
`ops/soradns/static_zones.<hostname>.json` كمقتطف من المحلل.

| المتغير / العلم | اقتراح |
| --- | --- |
| `DNS_ZONEFILE_OUT`، `--dns-zonefile-out` | كيفية إنشاء حجم ملف المنطقة. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`، `--dns-zonefile-resolver-snippet` | مسار مقتطف الحل (الافتراضي: `ops/soradns/static_zones.<hostname>.json` عندما يتم حذفه). |
| `DNS_ZONEFILE_TTL`، `--dns-zonefile-ttl` | يتم تطبيق TTL على السجلات المولدة (الافتراضي: 600 ثانية). |
| `DNS_ZONEFILE_IPV4`، `--dns-zonefile-ipv4` | عناوين IPv4 (يتم فصلها بفواصل أو وضع علامة على CLI متكررة). |
| `DNS_ZONEFILE_IPV6`، `--dns-zonefile-ipv6` | اتجاهات IPv6. |
| `DNS_ZONEFILE_CNAME`، `--dns-zonefile-cname` | استهداف CNAME اختياري. |
| `DNS_ZONEFILE_SPKI`، `--dns-zonefile-spki-pin` | الصنوبر SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`، `--dns-zonefile-txt` | إدخالات TXT إضافية (`key=value`). |
| `DNS_ZONEFILE_VERSION`، `--dns-zonefile-version` | تجاوز تسمية إصدار Zonefile المحوسب. |
| `DNS_ZONEFILE_EFFECTIVE_AT`، `--dns-zonefile-effective-at` | اضغط على الطابع الزمني `effective_at` (RFC3339) في مكان فتح النافذة المقطوعة. |
| `DNS_ZONEFILE_PROOF`، `--dns-zonefile-proof` | تجاوز الدليل الحرفي المسجل في بيانات التعريف. |
| `DNS_ZONEFILE_CID`، `--dns-zonefile-cid` | تجاوز تسجيل CID في بيانات التعريف. |
| `DNS_ZONEFILE_FREEZE_STATE`، `--dns-zonefile-freeze-state` | حالة التجميد (الناعم، الصلب، الذوبان، المراقبة، الطوارئ). |
| `DNS_ZONEFILE_FREEZE_TICKET`، `--dns-zonefile-freeze-ticket` | مرجع تذكرة الوصي/المجلس للتجميد. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`، `--dns-zonefile-freeze-expires-at` | الطابع الزمني RFC3339 لإذابة الجليد. |
| `DNS_ZONEFILE_FREEZE_NOTES`، `--dns-zonefile-freeze-note` | ملاحظات التجميد الإضافية (مفصولة بفواصل أو علم متكرر). |
| `DNS_GAR_DIGEST`، `--dns-gar-digest` | ملخص BLAKE3-256 (ست عشري) للحمولة الثابتة GAR. يلزم وجود روابط البوابة. |

يوفر سير عمل GitHub Actions هذه القيم من خلال أسرار المستودع لكل دبوس
يتم إنتاج منتجات Zonefile تلقائيًا. تكوين الأسرار التالية
(السلاسل تحتوي على قوائم منفصلة بفواصل متعددة المجالات):

| سر | اقتراح |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`، `DOCS_SORAFS_DNS_ZONE` | اسم المضيف/منطقة الإنتاج التي تمتد إلى المساعد. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | الاسم المستعار عند الطلب في الواصف. |
| `DOCS_SORAFS_ZONEFILE_IPV4`، `DOCS_SORAFS_ZONEFILE_IPV6` | تسجيل IPv4/IPv6 منشور. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | استهداف CNAME اختياري. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | باينز SPKI قاعدة64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | إدخالات TXT إضافية. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | تم تسجيل بيانات التعريف الخاصة بالتجميد بشكل ثابت. |
| `DOCS_SORAFS_GAR_DIGEST` | ملخص BLAKE3 في الحمولة السداسية GAR الثابتة. |

على الرغم من `.github/workflows/docs-portal-sorafs-pin.yml`، يتناسب مع المدخلات
`dns_change_ticket` و`dns_cutover_window` لكي يتم وضع الواصف/ملف المنطقة على النافذة
صحيح. Dejarlos em blanco apenasquando ejacutes dre runs.

نوع الاستدعاء (يتزامن مع مالك دفتر التشغيل SN-7):

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
 --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
 --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
 --dns-zonefile-ipv4 198.51.100.4 \
 --dns-zonefile-ttl 600 \
 --dns-zonefile-freeze-state soft \
 --dns-zonefile-freeze-ticket SNS-DF-XXXX \
 --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
 --dns-gar-digest <gar-digest-hex> \
 ...otros flags...
```

يقوم المساعد تلقائيًا بسحب تذكرة التغيير مثل إدخال TXT وإدراج أو بدء
نافذة القطع مثل الطابع الزمني `effective_at` مما يجب تجاوزه. الفقرة أو التدفق
العملية كاملة، الإصدار `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Nota sobre delegacao DNS publica

يحدد الهيكل العظمي لملف المنطقة جميع السجلات التلقائية للمنطقة. آيندا إي
من الضروري تكوين مفوض NS/DS للمنطقة دون المسجل أو المثبت
DNS حتى يتمكن الإنترنت من العثور على خوادم الأسماء.

- للقطع بدون قمة/TLD، استخدم ALIAS/ANAME (يعتمد على المثبت) أو للنشر
  يتم تسجيل A/AAAA لبوابة نظام التشغيل IPs Anycast.
- بالنسبة للمجالات الفرعية، يتم نشر CNAME لمضيف جميل مشتق
  (`<fqdn>.gw.sora.name`).
- يا مضيف Canonico (`<hash>.gw.sora.id`) يدوم طويلاً o dominio do gate e nao
  يتم النشر في منطقتك العامة.

### غرس رؤوس البوابة

O helper deploe Tambem Emite `portal.gateway.headers.txt` e
`portal.gateway.binding.json`، المصنوعات اليدوية التي تلبي متطلبات DG-3
ربط محتوى البوابة:

- يحتوي `portal.gateway.headers.txt` على كتلة كاملة من رؤوس HTTP (بما في ذلك
 `Sora-Name`، `Sora-Content-CID`، `Sora-Proof`، CSP، HSTS، واصف
 `Sora-Route-Binding`) يجب أن يتم جذب بوابات الحدود إلى كل استجابة.
- `portal.gateway.binding.json` قم بتسجيل معلومات واضحة بطريقة مقروءة للآلات
 لكي تتمكن تذاكر التغيير والأتمتة من مقارنة الارتباطات المضيفة/CID SEM
 raspar وخرج من القشرة.

يتم إنشاءه تلقائيًا عبر
`cargo xtask soradns-binding-template`
قم بالتقاط الاسم المستعار أو ملخص البيان أو اسم المضيف للبوابة التي تريدها
باسارون `sorafs-pin-release.sh`. لتجديد أو تخصيص أو كتلة الرؤوس،
تنفيذ:

```bash
cargo xtask soradns-binding-template \
 --manifest artifacts/sorafs/portal.manifest.json \
 --alias docs.sora \
 --hostname docs.sora.link \
 --route-label production \
 --json-out artifacts/sorafs/portal.gateway.binding.json \
 --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Pasa `--csp-template`، `--permissions-template`، o `--hsts-template` للتجاوز
قوالب الرؤوس الافتراضية عندما يتطلب النشر توجيهات إضافية؛
توجد مجموعات مع مفاتيح التشغيل `--no-*` لإزالة الرأس بالكامل.قم بإرفاق مقتطف من الرؤوس لطلب تغيير CDN وإمداد مستند JSON
خط الأنابيب الآلي للبوابة بحيث يتزامن الترويج الحقيقي للمضيف مع a
أدلة الإفراج.

يتم تنفيذ نص الإصدار تلقائيًا أو مساعد التحقق منه
تذاكر DG-3 تتضمن الأدلة الحديثة. قم بالتشغيل يدويًا
عند إجراء التعديلات أو ربط JSON يدويًا:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

قم بفك تشفير الحمولة `Sora-Proof`، تأكد من بيانات التعريف `Sora-Route-Binding`
يتزامن مع بيان CID + اسم المضيف، ويسقط بسرعة إذا تم حذف أي رأس.
قم بإخراج وحدة التحكم إلى جانب العناصر الأخرى التي يتم تشغيلها دائمًا
o أمر فوري من CI لكي تثبت مراجعي DG-3 أن الارتباط صالح بالفعل
ما قبل القطع.

> **تكامل واصف DNS:** `portal.dns-cutover.json` الآن يتم إضافة قسم إليه
> `gateway_binding` يشير إلى هذه المصنوعات (المسارات والمحتوى CID وحالة الإثبات وما إلى ذلك)
> قالب الرؤوس الحرفي) **e** مرجع `route_plan`
> `gateway.route_plan.json` المزيد من قوالب الرؤوس الرئيسية والتراجع. تشمل هذا
> كتل في كل تذكرة تغيير DG-3 حتى يتمكن المراجعون من مقارنة القيم
> تفاصيل `Sora-Name/Sora-Proof/CSP` وتأكيد خطط الترويج/التراجع
> يتزامن مع حزمة الأدلة التي لم يتم فتحها أو إنشاء ملف لها.

## الخطوة 9 - تنفيذ عمليات المراقبة العامة

يتطلب عنصر خريطة الطريق **DOCS-3c** أدلة مستمرة حول البوابة أو الوكيل أو نظام التشغيل
يتم الاحتفاظ بربطات البوابة بشكل جيد بعد تحريرها. تنفيذ أو مراقبة الدمج
مباشرة بعد الخطوات 7-8 والاتصال ببرامج التحقيق الخاصة بك:

```bash
cd docs/portal
npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%E%m%dT%H%M%SZ).json \
 --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` شحن أو ملف التكوين (الإصدار
 `docs/portal/docs/devportal/publishing-monitoring.md` للمشكلة) ه
 تنفيذ ثلاثة اختبارات: تحقيقات مسارات البوابة + التحقق من سياسة CSP/Permissions،
 تحقيقات الوكيل قم بفحصها (اختياريًا لنقطة النهاية `/metrics`)، والمتحقق
 ربط البوابة (`cargo xtask soradns-verify-binding`) الذي يتطلب الآن وجودًا
 القيمة المتوقعة من Sora-Content-CID جنبًا إلى جنب مع عمليات التحقق من الاسم المستعار/البيان.
- قم بإنهاء الأمر باستخدام غير صفر عندما يفشل أحد المسبار لـ CI، أو وظائف cron، أو المشغلين
 يمكن لـ runbook أن يوقف الإصدار قبل الترويج للاسم المستعار.
- Pasar `--json-out` يكتب ملخص JSON unico com estado por target؛ `--evidence-dir`
 انبعاث `summary.json` و`portal.json` و`tryit.json` و`binding.json` و`checksums.sha256` لذلك
 يمكن لمراجعي الحوكمة مقارنة النتائج دون إعادة تنفيذ المراقبين. أركيف
 هذا الدليل bajo `artifacts/sorafs/<tag>/monitoring/` junto al package Sigstore e o
 واصف قطع DNS.
- تضمين شاشة الإخراج، أو تصدير Grafana (`dashboards/grafana/docs_portal.json`)،
 e o ID del Drill of Alertmanager em o تذكرة الإصدار حتى يتمكن o SLO DOCS-3c من العمل
 com.auditado luego. كتاب اللعب المخصص لمراقبة النشر المباشر
 `docs/portal/docs/devportal/publishing-monitoring.md`.

تتطلب تحقيقات البوابة HTTPS وإرجاع عناوين URL الأساسية `http://` إلى `allowInsecureHttp`
تم تكوينه على شاشة التكوين؛ نحافظ على أهداف الإنتاج/العرض على TLS وقليلًا
القدرة على تجاوز معاينة اللغات.

أتمتة الشاشة عبر `npm run monitor:publishing` في Buildkite/cron بمجرد فتح البوابة
هذا هو الجسم الحي. في نفس الوقت، قم بإضافة عناوين URL للإنتاج، وقم بتشغيل فحوصات الصحة
الاستمرار في استخدام SRE/Docs لجميع الإصدارات.

## أوتوماكاو كوم `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` مغلف os Etapas 2-6. إستي:

1. قم بالوصول إلى `build/` في تحديد كرة القطران،
2. تنفيذ `car pack`، `manifest build`، `manifest sign`، `manifest verify-signature`،
 هـ `proof verify`,
3. تنفيذ `manifest submit` اختياريًا (بما في ذلك ربط الاسم المستعار) عند الاعتماد
 Torii، ه
4. أكتب `artifacts/sorafs/portal.pin.report.json`، أو اختياري
 `portal.pin.proposal.json`، واصف DNS المقطوع (بعد التقديمات)،
 e o حزمة ربط البوابة (`portal.gateway.binding.json` mas o كتلة الرؤوس)
 لكي تتمكن أجهزة الحوكمة والشبكات والعمليات من مقارنة حزمة الأدلة
 SEM revisar logs de CI.

التكوين `PIN_ALIAS`، `PIN_ALIAS_NAMESPACE`، `PIN_ALIAS_NAME`، e (اختياري)
`PIN_ALIAS_PROOF_PATH` قبل استدعاء البرنامج النصي. الولايات المتحدة الأمريكية `--skip-submit` جافة
يدير؛ يتم وصف سير عمل GitHub مرة أخرى عبر الإدخال `perform_submit`.

## Etapa 8 - الحزم العامة المتخصصة OpenAPI وحزم SBOM

يتطلب DOCS-7 إنشاء البوابة الإلكترونية ومواصفات OpenAPI وعناصر SBOM المنقولة
من أجل تحديد خط الأنابيب. المساعدون موجودون في ثلاثة أماكن:

1. **التجديد والتركيب حسب المواصفات.**

 ```bash
 npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
 cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
 ```

 اتبع إرشادات الإصدار عبر `--version=<label>` عندما تريد الاحتفاظ بلقطة
 تاريخي (على سبيل المثال `2025-q3`). يا مساعد اكتب يا لقطة م
 `static/openapi/versions/<label>/torii.json`، أنظر إليهم
 `versions/current`، وتسجيل البيانات التعريفية (SHA-256، حالة البيان، e
 تم تحديث الطابع الزمني) في `static/openapi/versions.json`. يا بوابة المصممين
 هذا مؤشر على أن لوحات Swagger/RapiDoc يمكنها تقديم محدد الإصدار
 وإظهار الملخص/الدمج المرتبط بالخط. Omitir `--version` mantem as del etiquetas del
 الإصدار السابق سليم وجديد تمامًا للنقاط `current` + `latest`.

 يلخص البيان SHA-256/BLAKE3 حتى تتمكن من استيعاب البوابة
 الرؤوس `Sora-Proof` لـ `/reference/torii-swagger`.

2. **Emite SBOMs CycloneDX.** خط الأنابيب لتحرير SBOMs استنادًا إلى النظام
 سيجون `docs/source/sorafs_release_pipeline_plan.md`. استمر في الخلاص
 جنبًا إلى جنب مع قطع البناء:

 ```bash
 syft dir:build -o json > "$OUT"/portal.sbom.json
 syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
 ```

3. **تجميع الحمولة الصافية في السيارة.**

 ```bash
 sorafs_cli car pack \
 --input docs/portal/static/openapi \
 --car-out "$OUT"/openapi.car \
 --plan-out "$OUT"/openapi.plan.json \
 --summary-out "$OUT"/openapi.car.json

 sorafs_cli car pack \
 --input "$OUT"/portal.sbom.json \
 --car-out "$OUT"/portal.sbom.car \
 --plan-out "$OUT"/portal.sbom.plan.json \
 --summary-out "$OUT"/portal.sbom.car.json
 ```

 اتبع الخطوات التالية في `manifest build` / `manifest sign` في الموقع الرئيسي،
 تعديل الاسم المستعار للمنتج (على سبيل المثال، `docs-openapi.sora` للمواصفات
 `docs-sbom.sora` لحزمة SBOM الثابتة). الاسم المستعار Manter distintos mantem SoraDNS،
 GARs وتذاكر التراجع عن الحمولة تمامًا.

4. **إرسال وربط.** إعادة استخدام Autoridad الموجود وحزمة Sigstore، فقط للتسجيل
 مجموعة الاسم المستعار في قائمة المراجعة التي يمكن للمراجعين من خلالها تحديد اسم سورا
 خريطة لكي هضم البيان.

قم بحفظ بيانات المواصفات/SBOM جنبًا إلى جنب مع إنشاء البوابة للتأكد من كل تذكرة
قم بتحرير المحتوى أو مجموعة العناصر الكاملة دون إعادة تنفيذها أو التعبئة.

### مساعد تلقائي (برنامج نصي CI/package)

`./ci/package_docs_portal_sorafs.sh` تدوين الخطوات 1-8 لعنصر خريطة الطريق
**DOCS-7** يمكنك تشغيله بأمر بسيط. أيها المساعد:

- تنفيذ طلب إعداد البوابة (`npm ci`، مزامنة OpenAPI/norito، اختبارات عناصر واجهة المستخدم)؛
- قم بإصدار قوائم السيارات وبيانات البوابة، OpenAPI وSBOM عبر `sorafs_cli`؛
- تنفيذ `sorafs_cli proof verify` (`--proof`) واختياريًا Sigstore
 (`--sign`، `--sigstore-provider`، `--sigstore-audience`)؛
- deja todos os artefactos bajo `artifacts/devportal/sorafs/<timestamp>/` e
 اكتب `package_summary.json` حتى تتمكن CI/tooling من دمج الحزمة؛ ه
- قم بتحديث `artifacts/devportal/sorafs/latest` للتنفيذ مؤخرًا.

مثال (خط الأنابيب مكتمل مع Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
 --proof \
 --sign \
 --sigstore-provider=github-actions \
 --sigstore-audience=sorafs-devportal
```

أعلام الكونسيرج:

- `--out <dir>` - تجاوز جذر المصنوعات اليدوية (الطابع الزمني الافتراضي لسجادة com).
- `--skip-build` - إعادة استخدام `docs/portal/build` الموجود (عند استخدام CI بدون وضع
 إعادة بناء المرايا دون اتصال بالإنترنت).
- `--skip-sync-openapi` - حذف `npm run sync-openapi` عندما `cargo xtask openapi`
 لا يمكن أن يكون مجرد صناديق.io.
- `--skip-sbom` - قم بالاتصال بـ `syft` عند عدم تثبيت الملف الثنائي (أو طباعة البرنامج النصي)
 إعلان ما في مكانك).
- `--proof` - تنفيذ `sorafs_cli proof verify` لكل سيارة/بيان. الحمولات السائبة دي
 تتطلب الأرشيفات المتعددة دعم خطة القطع في CLI، كما هو الحال مع هذه العلامة
 لم يتم التثبيت إذا تم اكتشاف أخطاء `plan chunk count` والتحقق يدويًا عندما
 llegue يا بوابة المنبع.
- `--sign` - الاستدعاء `sorafs_cli manifest sign`. Provee um token com
 `SIGSTORE_ID_TOKEN` (o `--sigstore-token-env`) أو دع سطر الأوامر يحصل على الاستخدام
 `--sigstore-provider/--sigstore-audience`.

عندما تحسد على المصنوعات اليدوية من الولايات المتحدة الأمريكية `docs/portal/scripts/sorafs-pin-release.sh`.
الآن تحتوي على بوابة، OpenAPI وحمولات SBOM، وتجميع كل بيان، وتسجيل بيانات التعريف
الأصول الإضافية في `portal.additional_assets.json`. يساعد على فهم المقابض الاختيارية
يتم استخدام وحدة حزم CI بالإضافة إلى المحولات الجديدة `--openapi-*` و`--portal-sbom-*` وe
`--openapi-sbom-*` لتعيين مجموعات من الأسماء المستعارة للمنتج، وتجاوز مصدر SBOM عبر
`--openapi-sbom-source`، الحمولات الصافية المحذوفة (`--skip-openapi`/`--skip-sbom`)،
قم بفتح الملف الثنائي `syft` بدون com الافتراضي `--syft-bin`.

يعرض البرنامج النصي كل أمر يتم تنفيذه؛ نسخة أو سجل أو تذكرة الإصدار
جنبًا إلى جنب مع `package_summary.json` حتى تتمكن المراجعون من مقارنة ملخصات CAR وبيانات التعريف
الخطة، وتجزئة الحزمة Sigstore لمراجعة الصدفة بشكل خاص.

## المرحلة 9 - التحقق من البوابة + SoraDNS

قبل الإعلان عن عملية القطع، إثبات أن الاسم المستعار الجديد يتم إرساله عبر SoraDNS وما هي البوابات
إنغرابان بروفاس فريسكا:

1. **تنفيذ بوابة المسبار.** `ci/check_sorafs_gateway_probe.sh` ejercita
 `cargo xtask sorafs-gateway-probe` عرض توضيحي لتركيبات نظام التشغيل
 `fixtures/sorafs_gateway/probe_demo/`. لتصفح الواقع، قم بالتحقق من اسم المضيف الهدف:```bash
 ./ci/check_sorafs_gateway_probe.sh -- \
 --gatewae "https://docs.sora/.well-known/sorafs/manifest" \
 --header "Accept: application/json" \
 --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
 --gar-kee "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
 --host "docs.sora" \
 --report-json artifacts/sorafs_gateway_probe/ci/docs.json
 ```

 مسبار فك التشفير `Sora-Name`، `Sora-Proof`، و`Sora-Proof-Status` ثانية
 `docs/source/sorafs_alias_policy.md` ويفشل عند تلخيص البيان،
 سيتم إنشاء روابط TTL أو روابط GAR.

   لإجراء فحوصات مفاجئة خفيفة الوزن (على سبيل المثال، عند ربط الحزمة فقط
   تم التغيير)، قم بتشغيل `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   يقوم المساعد بالتحقق من صحة حزمة الربط التي تم التقاطها وهو سهل الإصدار
   التذاكر التي تحتاج فقط إلى تأكيد ملزم بدلاً من تدريبات التحقيق الكاملة.

2. **التقاط أدلة التدريبات.** لتدريبات المشغل أو محاكاة PagerDuty، قم بإرفاقها
 o دقق في com `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
 طرح devportal -- ...`. أو غلاف يحرس الرؤوس/السجلات
 `artifacts/sorafs_gateway_probe/<stamp>/`، التحديث `ops/drill-log.md`، ه
 (اختياريًا) يفصل خطافات التراجع أو الحمولات النافعة PagerDuty. إستابليس
 `--host docs.sora` للتحقق من طريق SoraDNS في مكان التشفير الثابت لعنوان IP.

3. **التحقق من روابط DNS.** عند نشر الإدارة أو إثبات الاسم المستعار أو التسجيل أو أرشيف GAR
 تمت الإشارة إليه بواسطة المسبار (`--gar`) وإضافته إلى دليل الإصدار. أصحاب الحل
 يمكنك إعادة تشغيل نفس الإدخال عبر `tools/soradns-resolver` لضمان إدخال ذاكرة التخزين المؤقت
 احترام البيان الجديد. قبل إضافة JSON، قم بالتنفيذ
 `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
 حتى تتمكن من تحديد المضيف وبيانات التعريف وبيانات جهاز القياس عن بعد
 صالح حاليا. يمكن للمساعد إرسال ملخص `--json-out` إلى GAR الثابت لنظام التشغيل
 تحتوي المراجعات على أدلة يمكن التحقق منها دون فتح أو ثنائي.
 عندما يقوم بتنقيح GAR novo، تفضل
 `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
 (قم بعرض `--manifest-cid <cid>` بمجرد أن يكون ملف البيان غير متاح). يا مساعد
 الآن اشتقاق CID **e** o ملخص BLAKE3 مباشرة من بيان JSON، قم بقص المسافات على بياض،
 علامات إزالة التكرارات `--telemetry-label` المتكررة، وترتيب العلامات، وإصدار قوالب نظام التشغيل بشكل افتراضي
 de CSP/HSTS/Permissions-Police قبل كتابة JSON للحفاظ على تحديد الحمولة
 بما في ذلك عندما يلتقط المشغلون السمات من الأصداف المختلفة.

4. **لاحظ مقاييس الاسم المستعار.** Mantenha `torii_sorafs_alias_cache_refresh_duration_ms`
 e `torii_sorafs_gateway_refusals_total{profile="docs"}` على الشاشة أثناء المسبار
 كور؛ تم تحميل السلسلة الرسومية على `dashboards/grafana/docs_portal.json`.

## الحلقة 10 - المراقبة وحزمة الأدلة

- **لوحات المعلومات.** تصدير `dashboards/grafana/docs_portal.json` (SLOs للبوابة)،
 `dashboards/grafana/sorafs_gateway_observability.json` (زمن الوصول للبوابة +
 صحة الإثبات)، `dashboards/grafana/sorafs_fetch_observability.json`
 (salud del Orchestra) لكل إصدار. يقوم Anexe OS بتصدير JSON al Ticket de
 حرر حتى تتمكن المراجع من إعادة إنتاج الاستعلامات الخاصة بـ Prometheus.
- **ملفات المسبار.** حفظ `artifacts/sorafs_gateway_probe/<stamp>/` في git-annex
 يا دلو الأدلة. يتضمن ملخص المسبار والرؤوس والحمولة النافعة PagerDuty
 تم التقاطه بواسطة نص القياس عن بعد.
- **إصدار الحزمة.** حماية سيرة السيارة من البوابة/SBOM/OpenAPI، وحزم البيانات،
 Sigstore، `portal.pin.report.json`، سجلات اختبار Try-It، وتقارير التحقق من الارتباط
 الطابع الزمني لسجادة Bajo uma (على سبيل المثال، `artifacts/sorafs/devportal/20260212T1103Z/`).
- **سجل الحفر.** عندما تكون التحقيقات جزءًا من الحفر، قم بذلك
 `scripts/telemetry/run_sorafs_gateway_probe.sh` يجمع المدخلات مع `ops/drill-log.md`
 حتى يتم إثبات ذلك بشكل يرضي متطلبات SNNet-5.
- **روابط التذكرة.** راجع معرفات اللوحة Grafana أو تصدير PNG الملحقة للتذكرة
 من خلال التغيير، جنبًا إلى جنب مع طريق تقرير التحقيق، حتى تتمكن المراجعين من الوصول إلى SLOs
 لا يمكن الوصول إلى قذيفة.

## الخطوة 11 - أداة جلب أدلة متعددة المصادر ولوحة النتائج

نشر SoraFS الآن يتطلب أدلة جلب متعددة المصادر (DOCS-7/SF-6)
جنبًا إلى جنب مع اختبار DNS/البوابة السابقة. بعد تقديم البيان:

1. **تنفيذ `sorafs_fetch` مقابل البيان المباشر.** استخدام كل من عناصر الخطة/البيان
 تم إنتاجه في 2-3 سنوات من شهادات البوابات الصادرة لكل مورد. الثبات
 كل ذلك حتى يتمكن المستمعون من إعادة إنتاج نقطة قرار المنسق:

 ```bash
 OUT=artifacts/sorafs/devportal
 FETCH_OUT="$OUT/fetch/$(date -u +%E%m%dT%H%M%SZ)"
 mkdir -p "$FETCH_OUT"

 cargo run -p sorafs_car --bin sorafs_fetch -- \
 --plan "$OUT/portal.plan.json" \
 --manifest-json "$OUT/portal.manifest.json" \
 --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
 --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
 --scoreboard-out "$FETCH_OUT/scoreboard.json" \
 --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
 --json-out "$FETCH_OUT/fetch.json" \
 --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
 --telemetry-json artifacts/sorafs/provider_telemetry.json \
 --max-peers=3 \
 --retry-budget=4
 ```

 - ابحث أولاً عن إعلانات الموردين المشار إليها بالبيان (على سبيل المثال
 `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
 والخطوات عبر `--provider-advert name=path` حتى تتمكن لوحة النتائج من تقييم النوافذ
 القدرة على شكل محدد. الولايات المتحدة الأمريكية
 `--allow-implicit-provider-metadata` **apenas** عندما يقوم بإعادة إنتاج التركيبات em CI; تدريبات نظام التشغيل
 يعتمد الإنتاج على الإعلانات التجارية التي يتم تجميعها عبر الإنترنت.
 - عند الإشارة إلى مناطق إضافية في البيان، كرر الأمر مع مجموعات من
 الموردون المراسلون لكي تتمكن كل ذاكرة تخزين مؤقت/اسم مستعار من جلب قطعة أثرية مرتبطة.

2. **Arquive as Salidas.** Guarda `scoreboard.json`,
 `providers.ndjson`، `fetch.json`، و`chunk_receipts.ndjson` بعد تقديم الأدلة
 الافراج. تلتقط هذه المحفوظات أو ترجيح الأقران أو تراجع الميزانية أو زمن استجابة EWMA أو EWMA
 إيصالات جزء من حزمة الإدارة يجب الاحتفاظ بها لـ SF-7.

3. **تحديث القياس عن بعد.** استيراد نتائج الجلب إلى لوحة المعلومات **SoraFS Fetch
 إمكانية الملاحظة** (`dashboards/grafana/sorafs_fetch_observability.json`)،
 مراقبة `torii_sorafs_fetch_duration_ms`/`_failures_total` وألواح النطاق من قبل المورّد
 بارا الشذوذ. قم بإدراج لقطات اللوحة Grafana في تذكرة الإصدار جنبًا إلى جنب مع المسار
 لوحة النتائج.

4. **Smokea كأنظمة تنبيه.** نفذ `scripts/telemetry/test_sorafs_fetch_alerts.sh`
 للتحقق من حزمة التنبيهات Prometheus قبل الإغلاق أو الإصدار. ملحق الخروج
 أداة الترويج للتذكرة لتأكيد مراجع DOCS-7 على أنها تنبيهات للتوقف
 أسطول siguen مزود بطيء.

5. **كابل CI.** سير العمل من دبوس البوابة يتحكم في `sorafs_fetch` من الإدخال
 `perform_fetch_probe`; تأهيل لتنفيذ العرض المسرحي/الإنتاج كدليل على ذلك
 قم بإحضار المنتج جنبًا إلى جنب مع حزمة البيان دون دليل التدخل. نظام التشغيل تدريبات اللغات podem
 إعادة استخدام البرنامج النصي نفسه لتصدير الرموز المميزة للبوابة وتثبيتها `PIN_FETCH_PROVIDERS`
 قائمة الموردين المنفصلة بفواصل.

## الترويج وإمكانية المراقبة والتراجع

1. **الترويج:** احتفظ بالاسم المستعار المنفصل للإخراج والإنتاج. إعادة تنفيذ الترويج
 `manifest submit` مع البيان/الحزمة، التغيير
 `--alias-namespace/--alias-name` لتطوير الاسم المستعار للإنتاج. هذه هي الحياة
 إعادة بناء أو إعادة بناء شيء ما عندما يتم ضبط ضمان الجودة أو تثبيته.
2. **المراقبة:** استيراد لوحة معلومات التسجيل
 (`docs/source/grafana_sorafs_pin_registry.json`) يستكشف نظام التشغيل الخاص بالبوابة
 (الإصدار `docs/portal/docs/devportal/observability.md`). تنبيه em الانجراف دي المجموع الاختباري،
 تحقيقات Fallidos أو picos de retre de إثبات.
3. **التراجع:** للرجوع أو المراجعة أو البيان السابق (أو التراجع أو الاسم المستعار الفعلي) للاستخدام
 `sorafs_cli manifest submit --alias ... --retire`. الحفاظ على الحزمة دائمًا أو حتى النهاية
 أعلم أنه جيد واستئناف CAR حتى تتمكن من إعادة المحاولة أثناء تجربة التراجع
 سجلات نظام التشغيل CI نفسها روتان.

## مصنع سير العمل CI

كم الحد الأدنى، خط الأنابيب الخاص بك:

1. البناء + الوبر (`npm ci`، `npm run build`، توليد المجموع الاختباري).
2. Empacotar (`car pack`) وبيانات الكمبيوتر.
3. استخدم الرمز المميز OIDC للمهمة (`manifest sign`).
4. المصنوعات اليدوية الفرعية (السيارة، البيان، الحزمة، الخطة، الملخصات) للاستماع.
5. Enviar آل دبوس التسجيل:
 - سحب الطلبات -> `docs-preview.sora`.
 - العلامات / الفروع المحمية -> الترويج للاسم المستعار للإنتاج.
6. تنفيذ المجسات + بوابات التحقق من الإثبات قبل الانتهاء.

`.github/workflows/docs-portal-sorafs-pin.yml` قم بتوصيل جميع هذه الخطوات لإصدارات الأدلة.
يا سير العمل:

- البناء/الفحص عبر البوابة،
- empaqueta o build via `scripts/sorafs-pin-release.sh`،
- التثبيت/التحقق من حزمة البيان باستخدام GitHub OIDC،
- sube o CAR/manifesto/bundle/plan/resumos como artifacts، e
- (اختياريًا) يحسدون على البيان + الاسم المستعار الملزم عندما تكون لديهم أسرار.

قم بتكوين أسرار/متغيرات المستودع التالية قبل فقدان الوظيفة:

| الاسم | اقتراح |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | المضيف Torii الذي يعرض `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | معرف العصر المسجل مع التقديمات. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | عملية القتل التلقائي لتقديم البيان. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | تم إرفاق مجموعة الاسم المستعار بالبيان عندما `perform_submit` هو `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | حزمة إثبات الاسم المستعار المشفر في base64 (اختياري؛ حذف للربط بالاسم المستعار). |
| `DOCS_ANALYTICS_*` | يتم إعادة استخدام نقاط النهاية للتحليلات/التحقق من وجودها من خلال سير عمل آخر. |

تغيير سير العمل عبر واجهة المستخدم الخاصة بالإجراءات:

1. إثبات `alias_label` (على سبيل المثال، `docs.sora.link`)، `proposal_alias` اختياريًا،
 يمكنك تجاوز `release_tag` بشكل اختياري.
2. Deja `perform_submit` sem marcar لإنشاء المصنوعات اليدوية sem tocar Torii
 (استخدام للتشغيل) أو التمكن من النشر مباشرة على الاسم المستعار الذي تم تكوينه.

`docs/source/sorafs_ci_templates.md` أحد المستندات المساعدة لـ CI العامة
المشاريع فورًا في هذا المستودع، لكن سير عمل البوابة يجب أن يكون خيارًا مفضلاً
الفقرة النشرات ديل ديا ضياء.

## قائمة المراجعة- [ ] `npm run build`، `npm run test:*`، و`npm run check:links` موجودان باللون الأخضر.
- [ ] تم التقاط `build/checksums.sha256` و`build/release.json` من المصنوعات اليدوية.
- [ ] السيارة والخطة والبيان واستئناف العمل في `artifacts/`.
- [ ] الحزمة Sigstore + assinatura المنفصلة عن سجلات com.
- [ ] `portal.manifest.submit.summary.json` و`portal.manifest.submit.response.json`
 capturados quando hae التقديمات.
- [ ] `portal.pin.report.json` (اختياري `portal.pin.proposal.json`)
 أرشيفادو جنبًا إلى جنب مع نظام التشغيل المصنوعات اليدوية/البيان.
- [ ] سجلات أرشيفات `proof verify` و`manifest verify-signature`.
- [ ] لوحات معلومات Grafana تم تحديثها + تحقيقات Try-It خارجة.
- [ ] ملاحظات التراجع (معرف البيان السابق + ملخص الاسم المستعار) الملحقات الأخرى
 تذكرة الافراج.