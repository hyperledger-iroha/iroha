---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## بانوراما عامة

يقوم هذا الدليل بتحويل عناصر خريطة الطريق **DOCS-7** (منشور SoraFS) و **DOCS-8**
(أتمتة دبوس CI/CD) في إجراء مناسب لبوابة المطورين.
قم بتغطية مرحلة البناء/الوبر، التغليف SoraFS، الشركة المصنعة للبيانات مع Sigstore،
الترويج للاسم المستعار والتحقق وتدريبات التراجع حتى يتم معاينة كل إصدار وإصداره
البحر قابل للتكرار وقابل للتدقيق.

يفترض التدفق أن لديه الثنائي `sorafs_cli` (الذي تم إنشاؤه باستخدام `--features cli`)، قم بالوصول إلى
نقطة نهاية Torii مع أذونات تسجيل الدبوس، وبيانات الاعتماد OIDC لـ Sigstore. حراسة الأسرار
لحياة طويلة (`IROHA_PRIVATE_KEY`، `SIGSTORE_ID_TOKEN`، الرموز المميزة لـ Torii) في قبو CI؛ لاس
يمكن تنفيذ عمليات التنفيذ المحلية من خلال صادرات Shell.

## المتطلبات الأساسية

- العقدة 18.18+ مع `npm` أو `pnpm`.
- `sorafs_cli` من `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- عنوان URL الخاص بـ Torii الذي يعرض `/v1/sorafs/*` يحتوي على حساب/مفتاح خاص للمصدر يمكنك من خلاله إرسال البيانات والاسم المستعار.
- Emisor OIDC (GitHub Actions وGitLab وهوية عبء العمل وما إلى ذلك) لإصدار `SIGSTORE_ID_TOKEN`.
- اختياري: `examples/sorafs_cli_quickstart.sh` للتشغيل الجاف و`docs/source/sorafs_ci_templates.md` لدعم سير العمل في GitHub/GitLab.
- قم بتكوين متغيرات OAuth من Try it (`DOCS_OAUTH_*`) وقم بتشغيلها
  [قائمة التحقق من تعزيز الأمان](./security-hardening.md) قبل الترويج للإنشاء
  مختبر النار. بناء البوابة الآن سوف يسقط عندما تكون هذه المتغيرات فالتان
  عندما يتم تشغيل مقابض TTL/الاقتراع على النوافذ المطبقة؛ importa
  `DOCS_OAUTH_ALLOW_INSECURE=1` منفردًا لمعاينة الإعدادات المحلية القابلة للإلغاء. ملحق لا
  دليل اختبار القلم على تذكرة الإصدار.

## Paso 0 - التقاط حزمة من الوكيل جربها

قبل الترويج لمعاينة Netlify أو أي بوابة، قم ببيع مصادر الوكيل جربها بنفسك
ملخص البيان OpenAPI الثابت في حزمة محددة:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` نسخ مساعدي الوكيل/التحقيق/الاستعادة، التحقق من الشركة
OpenAPI واكتب `release.json` mas `checksums.sha256`. هذا الملحق هو حزمة التذكرة
الترويج لبوابة Netlify/SoraFS حتى تتمكن المراجعات من إعادة إنتاج العناصر الدقيقة
يتم إعادة بناء الوكيل ومسارات الهدف Torii. الحزمة تامبيان مسجلة si los
القائمون على توفير الدعم للعملاء المؤهلين (`allow_client_auth`) للحفاظ على الخطة
بدء التشغيل وأنظمة CSP والمزامنة.

## الخطوة 1 - قم ببناء البوابة

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

يتم تنفيذ `npm run build` تلقائيًا `scripts/write-checksums.mjs`، مما يؤدي إلى:

- `build/checksums.sha256` - يوضح SHA256 المناسب لـ `sha256sum -c`.
- `build/release.json` - البيانات الوصفية (`tag`، `generated_at`، `source`) موجودة في كل CAR/manifiersto.

أرشفة مجموعة كبيرة من الأرشيفات جنبًا إلى جنب مع السيرة الذاتية لـ CAR حتى يتمكن المراجعون من مقارنة المصنوعات اليدوية
معاينة دون إعادة البناء.

## الخطوة 2 - إمباكيتا الأصول الثابتة

قم بتشغيل أداة تعبئة السيارة مقابل مخرج Docusaurus. مثال العباءة
اكتب جميع القطع الأثرية bajo `artifacts/devportal/`.

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

تلتقط السيرة الذاتية لـ JSON محتويات القطع والملخصات والمسارات لتخطيط إثبات ذلك
`manifest build` ويتم إعادة استخدام لوحات معلومات CI بعد ذلك.

## باسو 2ب - رفاق إمباكيتا OpenAPI y SBOM

يتطلب DOCS-7 نشر موقع البوابة واللقطة OpenAPI والحمولات SBOM
كما تظهر هذه الميزات المميزة حتى تتمكن البوابات من حفر الرؤوس
`Sora-Proof`/`Sora-Content-CID` لكل قطعة أثرية. مساعد الافراج
(`scripts/sorafs-pin-release.sh`) لتعبئة الدليل OpenAPI
(`static/openapi/`) وSBOMs الصادرة عبر `syft` والسيارات المنفصلة
`openapi.*`/`*-sbom.*` وقم بتسجيل البيانات التعريفية
`artifacts/sorafs/portal.additional_assets.json`. تشغيل دليل التدفق,
كرر الخطوات 2-4 لكل حمولة مع تفضيلاتك وبيانات التعريف الخاصة بك
(على سبيل المثال `--car-out "$OUT"/openapi.car` ماس
`--metadata alias_label=docs.sora.link/openapi`). Registra cada par manificto/alias
في Torii (الموقع، OpenAPI، SBOM في البوابة، SBOM في OpenAPI) قبل تغيير DNS لذلك
يمكن للبوابة أن تخدم بشكل تجريبي لجميع القطع الأثرية المنشورة.

## الخطوة 3 - بناء البيان

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

قم بضبط الأعلام السياسية من خلال نافذة الإصدار الثانية (على سبيل المثال، `--pin-storage-class
hot` الفقرة الكناري). يعد متغير JSON اختياريًا ولكنه مناسب لمراجعة الكود.

## باسو 4 - Firma con Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

تسجل الحزمة ملخص البيان وملخصات القطع وتجزئة الرمز المميز BLAKE3
OIDC بدون استمرار JWT. حراسة هذه الحزمة كشركة منفصلة؛ العروض الترويجية دي
يمكن للإنتاج إعادة استخدام نفس المصنوعات اليدوية في مكان إعادة التثبيت. عمليات القذف المحلية
يمكنك استبدال علامات الموفر مع `--identity-token-env` (أو المُنشئ
`SIGSTORE_ID_TOKEN` في الداخل) أثناء وجود مساعد OIDC خارجيًا لإصدار الرمز المميز.

## Paso 5 - تسجيل Envia al pin

قم بإرسال البيان الثابت (وخطة القطع) إلى Torii. اطلب دائمًا استئنافًا لذلك
la entrada/alias resultante sea قابلة للتدقيق.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority i105... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

عند إرسال اسم مستعار لمعاينة الكناري (`docs-preview.sora`)، كرر ذلك
أرسل اسمًا مستعارًا فريدًا حتى تتمكن ضمان الجودة من التحقق من المحتوى قبل الترويج
إنتاج.

يتطلب التجليد المستعار ثلاثة مجالات: `--alias-namespace`، `--alias-name` و`--alias-proof`.
تقوم الحوكمة بإنتاج حزمة إثبات (base64 أو بايت Norito) عندما يتم تقديم الطلب
الاسم المستعار؛ قم بحماية أسرار CI واعرض مثل هذا الملف قبل استدعاء `manifest submit`.
قم بإدراج علامات الاسم المستعار بدون تثبيت عند الضغط فقط على إظهار البيان بدون استخدام DNS.

## الخطوة 5 ب - إنشاء نموذج للحوكمة

يجب على كل بيان أن يسافر من خلال قائمة مقترحة للبرلمان من أجل أي مدينة
يمكنك تقديم التغيير دون الحاجة إلى الحصول على امتيازات الاعتماد. بعد خطوة
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
ملخص القطع والسياسة ومسار الاسم المستعار. مساعد على تذكرة الحكم أو الجميع
بوابة البرلمان حتى يتمكن المندوبون من مقارنة الحمولة دون إعادة بناء المصنوعات اليدوية.
بما أن الأمر لم يضغط على المفتاح الرئيسي لـ Torii، فيمكن لأي مدينة تعديله
propuest localmente.

## الخطوة 6 - التحقق من الاختبار والقياس عن بعد

بعد الفتح، قم بتنفيذ خطوات التحقق المحددة:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- التحقق `torii_sorafs_gateway_refusals_total` ذ
  `torii_sorafs_replication_sla_total{outcome="missed"}` للشذوذ.
- قم بتشغيل `npm run probe:portal` لتشغيل الوكيل Try-It والروابط المسجلة
  مقابل المحتوى الذي يتم استلامه.
- التقاط الأدلة الموضحة في الشاشة
  [النشر والمراقبة](./publishing-monitoring.md) لبوابة المراقبة DOCS-3c
  إنه مرضي إلى جانب خطوات النشر. المساعد الآن يقبل مضاعفات المدخلات
  `bindings` (الموقع، OpenAPI، بوابة SBOM، SBOM في OpenAPI) ويتم تطبيق `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
  وهدف المضيف عبر الحارس الاختياري `hostname`. La invocacion de abajo escribe tanto un
  استئناف JSON كمجموعة من الأدلة (`portal.json`، `tryit.json`، `binding.json` y)
  `checksums.sha256`) خلف دليل الإصدار:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Paso 6a - مخطط البوابة المعتمد

اشتق خطة SAN/challenge TLS قبل إنشاء حزم GAR لمعدات البوابة والأشخاص
يقوم مطورو DNS بمراجعة الأدلة نفسها. يعكس المساعد الجديد إجراءات الأتمتة
يستضيف تعداد DG-3 رموز البدل الأساسية وشبكات SAN المضيفة الجميلة وخصائص DNS-01 والتحديات التي توصي بها ACME:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

قم بإدراج JSON جنبًا إلى جنب مع حزمة الإصدار (أو تحت تذكرة التغيير) للمشغلين
يمكنك تثبيت قيم SAN في التكوين `torii.sorafs_gateway.acme` de Torii والمراجعة
يمكن لـ GAR تأكيد الخرائط الكنسي/جميلة بدون إعادة تشغيل مشتقات المضيف. أجريجا
الحجج `--name` إضافية لكل ترويج صوفي في نفس الإصدار.

## Paso 6b - مشتق من خرائط المضيف الكنسي

قبل إنشاء حمولات GAR، قم بتسجيل خريطة المضيف المحددة لكل اسم مستعار.
`cargo xtask soradns-hosts` hashea cada `--name` على علامتك الكنسي
(`<base32>.gw.sora.id`)، قم بإنشاء حرف البدل المطلوب (`*.gw.sora.id`) واشتقاق المضيف الجميل
(`<alias>.gw.sora.name`). استمر في الخروج من عناصر التحرير حتى تتم المراجعة
يمكن DG-3 مقارنة الخريطة بجوار GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

الولايات المتحدة الأمريكية `--verify-host-patterns <file>` للسقوط السريع عند استخدام JSON de GAR أو ربط البوابة
حذف أحد المضيفين المطلوبين. المساعد يقبل عدة أرشيفات التحقق، ويزرع
من السهل الوبر على هذا النبات GAR مثل `portal.gateway.binding.json` المحفور في نفس الاستدعاء:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```قم بإضافة السيرة الذاتية لـ JSON وسجل التحقق من خلال تذكرة تغيير DNS/البوابة لذلك
يمكن للمراجعين تأكيد المضيفين الكنسيين وأحرف البدل وبدون إعادة تشغيل البرامج النصية.
أعد تنفيذ الأمر عند إضافة اسم مستعار جديد إلى الحزمة لتحديثات GAR
هنا نفس الدليل.

## الخطوة 7 - إنشاء واصف قطع DNS

تتطلب عمليات قطع الإنتاج حزمة من التغييرات قابلة للتدقيق. بعد الانتهاء من تقديم الخروج
(ملزم بالاسم المستعار)، المساعد يصدر
`artifacts/sorafs/portal.dns-cutover.json`، الالتقاط:

- بيانات التعريف المرتبطة بالاسم المستعار (مساحة الاسم/الاسم/الإثبات، ملخص البيان، URL Torii،
  عصر الإرسال، autoridad)؛
- سياق الإصدار (العلامة، التسمية المستعارة، مسارات البيان/CAR، خطة القطع، الحزمة Sigstore)؛
- نقاط التحقق (مسبار الأمر، الاسم المستعار + نقطة النهاية Torii)؛ ذ
- المجالات الاختيارية للتحكم في التغيير (معرف التذكرة، نافذة القطع، عمليات الاتصال،
  اسم المضيف/منطقة الإنتاج)؛
- بيانات التعريف الترويجية للطرق المشتقة من الرأس `Sora-Route-Binding`
  (المضيف Canonico/CID، مسارات الرأس + الربط، أوامر التحقق)، ضمان الترويج
  تشير GAR والتدريبات الاحتياطية إلى نفس الأدلة؛
- المصنوعات اليدوية لخطة الطريق التي تم إنشاؤها (`gateway.route_plan.json`،
  قوالب الرؤوس ورؤوس التراجع الاختيارية) لتذاكر التغيير والخطافات
  يمكن لخيط CI التحقق من أن كل حزمة DG-3 تشير إلى خطط الترويج/التراجع
  canonicos antes de la aprobacion؛
- البيانات التعريفية الاختيارية لإبطال ذاكرة التخزين المؤقت (نقطة نهاية التطهير، متغير المصادقة، الحمولة JSON،
  ومثال الأمر `curl`); ذ
- تلميحات التراجع عن الواصف السابق (علامة الإصدار وملخص البيان)
  لكي تلتقط التذاكر طريقًا احتياطيًا محددًا.

عندما يتطلب الإصدار إزالة ذاكرة التخزين المؤقت، يتم إنشاء خطة أساسية جنبًا إلى جنب مع واصف القطع:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

ملحق `portal.cache_plan.json` الناتج عن الحزمة DG-3 للمشغلين
يتم تحديد المضيفين/المسارات (وتلميحات المصادقة التي تتزامن) عن طريق إرسال طلبات `PURGE`.
يمكن للقسم الاختياري لذاكرة التخزين المؤقت للواصف الرجوع إلى هذا الأرشيف مباشرة،
الحفاظ على تحديثات التحكم في التغيير بشكل دقيق حتى يتم تنظيف نقاط النهاية
خلال عملية القطع.

تتطلب كل حزمة DG-3 أيضًا قائمة مرجعية للترويج + التراجع. جنرالا عبر
`cargo xtask soradns-route-plan` حتى تتمكن منشورات التحكم في التغيير من متابعة الخطوات
تفاصيل الاختبار المبدئي والانتقال والتراجع من خلال الاسم المستعار:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

يستضيف El `gateway.route_plan.json` emitido captura Canonicos/Pretty، مسجلات التحقق من الصحة
من خلال الخطوات وتحديثات ربط GAR وإزالة ذاكرة التخزين المؤقت وإجراءات التراجع. بما في ذلك يخدع
تم تصنيع GAR/الربط/القطع قبل إرسال تذكرة التغيير حتى تتمكن العملية من تجربتها
aprobar los mesmos pasos con guion.

`scripts/generate-dns-cutover-plan.mjs` هو الواصف الدافع ويتم تنفيذه تلقائيًا من هنا
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

قم باستعادة البيانات التعريفية الاختيارية عبر متغيرات الإعداد قبل تشغيل مساعد الدبوس:

| متغير | اقتراح |
| --- | --- |
| `DNS_CHANGE_TICKET` | معرف التذكرة مخزن في الواصف. |
| `DNS_CUTOVER_WINDOW` | فتحة التهوية ISO8601 (على سبيل المثال، `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`، `DNS_ZONE` | اسم المضيف للإنتاج + منطقة التشغيل التلقائية. |
| `DNS_OPS_CONTACT` | الاسم المستعار عند الطلب أو الاتصال بالتصعيد. |
| `DNS_CACHE_PURGE_ENDPOINT` | نقطة نهاية مسح ذاكرة التخزين المؤقت المسجلة في الواصف. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env هو الذي يحتوي على رمز التطهير (الافتراضي: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | استخدم واصف القطع السابق لبيانات التعريف الخاصة بالتراجع. |

قم بإضافة JSON إلى مراجعة تغيير DNS حتى يتمكن الموردون من التحقق من خلاصات البيان،
تستكشف روابط الأسماء المستعارة والأوامر دون مراجعة سجلات CI. أعلام CLI
`--dns-change-ticket`، `--dns-cutover-window`، `--dns-hostname`،
`--dns-zone`، `--ops-contact`، `--cache-purge-endpoint`،
`--cache-purge-auth-env`، و`--previous-dns-plan` أثبتا نفس التجاوزات
عند تشغيل مساعد CI القوي.

## الخطوة 8 - إنشاء ملف المنطقة من المحلل (اختياري)

عندما يتم تحديد نافذة قطع الإنتاج، يمكن أن يصدر نص الإصدار
قم بتجميع ملف المنطقة SNS ومقتطف الحل تلقائيًا. قم بمتابعة سجلات DNS المطلوبة
البيانات التعريفية عبر متغيرات المحتوى أو خيارات سطر الأوامر؛ المساعد لامارا أ
`scripts/sns_zonefile_skeleton.py` فورًا بعد إنشاء واصف القطع.
أثبت أقل قيمة من A/AAAA/CNAME وGAR (BLAKE3-256 من الحمولة الثابتة GAR). سي لا
المنطقة/اسم المضيف son conocidos y `--dns-zonefile-out` se حذفت، el helper escribe en
`artifacts/sns/zonefiles/<zone>/<hostname>.json` ولدينا
`ops/soradns/static_zones.<hostname>.json` كمقتطف المحلل.

| المتغير / العلم | اقتراح |
| --- | --- |
| `DNS_ZONEFILE_OUT`، `--dns-zonefile-out` | طريقة إنشاء ملف المنطقة. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`، `--dns-zonefile-resolver-snippet` | مسار مقتطف الحل (الافتراضي: `ops/soradns/static_zones.<hostname>.json` عند حذفه). |
| `DNS_ZONEFILE_TTL`، `--dns-zonefile-ttl` | تم تطبيق TTL على السجلات العامة (الافتراضي: 600 ثانية). |
| `DNS_ZONEFILE_IPV4`، `--dns-zonefile-ipv4` | عناوين IPv4 (يتم فصلها بفواصل أو وضع علامة على CLI متكررة). |
| `DNS_ZONEFILE_IPV6`، `--dns-zonefile-ipv6` | اتجاهات IPv6. |
| `DNS_ZONEFILE_CNAME`، `--dns-zonefile-cname` | استهداف CNAME اختياري. |
| `DNS_ZONEFILE_SPKI`، `--dns-zonefile-spki-pin` | الصنوبر SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`، `--dns-zonefile-txt` | إدخالات TXT إضافية (`key=value`). |
| `DNS_ZONEFILE_VERSION`، `--dns-zonefile-version` | تجاوز تسمية إصدار Zonefile المحوسب. |
| `DNS_ZONEFILE_EFFECTIVE_AT`، `--dns-zonefile-effective-at` | اضغط على الطابع الزمني `effective_at` (RFC3339) خلف بداية نافذة القطع. |
| `DNS_ZONEFILE_PROOF`، `--dns-zonefile-proof` | تجاوز الدليل الحرفي المسجل في بيانات التعريف. |
| `DNS_ZONEFILE_CID`، `--dns-zonefile-cid` | تجاوز تسجيل CID في البيانات الوصفية. |
| `DNS_ZONEFILE_FREEZE_STATE`، `--dns-zonefile-freeze-state` | حالة التجميد (الناعم، الصلب، الذوبان، المراقبة، الطوارئ). |
| `DNS_ZONEFILE_FREEZE_TICKET`، `--dns-zonefile-freeze-ticket` | مرجع تذكرة الوصي/المجلس للتجميد. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`، `--dns-zonefile-freeze-expires-at` | الطابع الزمني RFC3339 لإذابة الجليد. |
| `DNS_ZONEFILE_FREEZE_NOTES`، `--dns-zonefile-freeze-note` | ملاحظات التجميد الإضافية (مفصولة بفواصل أو علم متكرر). |
| `DNS_GAR_DIGEST`، `--dns-gar-digest` | ملخص BLAKE3-256 (ست عشري) للحمولة الثابتة GAR. يتطلب الأمر عند ربط البوابة. |

سير العمل في GitHub Actions يوضح هذه القيم من أسرار المستودع لكل دبوس
يتم إنتاج منتجات Zonefile تلقائيًا. أسرار تكوين المتابعين
(يمكن أن تحتوي السلاسل على قوائم منفصلة بفواصل متعددة المجالات):

| سر | اقتراح |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`، `DOCS_SORAFS_DNS_ZONE` | اسم المضيف/منطقة الإنتاج التي تمتد إلى المساعد. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | الاسم المستعار عند الطلب المخزن في الواصف. |
| `DOCS_SORAFS_ZONEFILE_IPV4`، `DOCS_SORAFS_ZONEFILE_IPV6` | تسجيل IPv4/IPv6 منشور. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | استهداف CNAME اختياري. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | باينز SPKI قاعدة64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | إدخالات TXT إضافية. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | تجميد البيانات الوصفية المسجلة على هذا النحو. |
| `DOCS_SORAFS_GAR_DIGEST` | ملخص BLAKE3 في الحمولة السداسية GAR الثابتة. |

على الرغم من `.github/workflows/docs-portal-sorafs-pin.yml`، يتناسب مع المدخلات
`dns_change_ticket` و `dns_cutover_window` لوضع الواصف/ملف المنطقة هنا على النافذة
صحيح. يقوم Dejarlos en blanco Solo cuando بإخراج الجري الجاف.

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

يقوم المساعد تلقائيًا بسحب تذكرة التغيير مثل إدخال TXT وإدراج بداية الصفحة
نافذة القطع مثل الطابع الزمني `effective_at` مما يجب تجاوزه. للرحلة
العملية كاملة، الإصدار `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Nota sobre delegacion DNS publica

يقوم الهيكل العظمي لملف المنطقة بتحديد السجلات التلقائية للمنطقة فقط. عون
يجب عليك تكوين تفويض NS/DS لمنطقة الأب ومسجلك
مزود DNS حتى يتمكن عام الإنترنت من العثور على خوادم الأسماء.

- للقطع في قمة/TLD، usa ALIAS/ANAME (تبعًا للمورد) o
  نشر السجلات A/AAAA التي تفتح بوابة IP لأي بث.
- بالنسبة للنطاقات الفرعية، قم بنشر CNAME بواسطة المضيف الجميل المشتق
  (`<fqdn>.gw.sora.name`).
- المضيف Canonico (`<hash>.gw.sora.id`) يدوم طويلاً تحت سيطرة البوابة
  ولن يتم النشر في منطقتك العامة.

### غرس رؤوس البوابة

مساعد النشر ينبعث `portal.gateway.headers.txt` y
`portal.gateway.binding.json`، هذه المصنوعات التي تلبي متطلبات DG-3
ربط محتوى البوابة:

- يحتوي `portal.gateway.headers.txt` على كتلة كاملة من رؤوس HTTP (بما في ذلك
  `Sora-Name`، `Sora-Content-CID`، `Sora-Proof`، CSP، HSTS، والواصف
  `Sora-Route-Binding`) يجب أن يتم جذب بوابات الحدود إلى كل استجابة.
- `portal.gateway.binding.json` تسجيل هذه المعلومات بشكل مقروء للآلات
  حتى تتمكن تذاكر التغيير والأتمتة من مقارنة الارتباطات مع المضيف/CID
  تخلص من القشرة.يتم إنشاءه تلقائيًا عبر
`cargo xtask soradns-binding-template`
والتقط الاسم المستعار وملخص البيان واسم مضيف البوابة الذي تريده
باسارون `sorafs-pin-release.sh`. لتجديد أو تخصيص كتلة الرؤوس،
القذف:

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
توجد مجموعات مع المفاتيح `--no-*` لإزالة الرأس بالكامل.

إضافة مقتطف الرؤوس إلى طلب تغيير CDN وتشغيل مستند JSON
خط أنابيب أتمتة البوابة بحيث يتزامن الترويج الحقيقي للمضيف مع ذلك
أدلة الإفراج.

يقوم نص الإصدار بتشغيل مساعد التحقق تلقائيًا لهم
تذاكر DG-3 تتضمن الأدلة الحديثة. قم بالتشغيل يدويًا
cuando يقوم بتحرير الربط JSON يدويًا:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

يتم فك تشفير الحمولة `Sora-Proof`، تأكد من بيانات التعريف `Sora-Route-Binding`
يتزامن مع بيان CID + اسم المضيف، ويسقط بسرعة إذا تم حذف أي رأس.
أرشفة خروج وحدة التحكم جنبًا إلى جنب مع العناصر الأخرى التي يتم تشغيلها دائمًا
الأمر فوري من CI لكي تكون مراجع DG-3 قيد الاختبار للتأكد من صلاحية الربط
ما قبل القطع.

> **تكامل واصف DNS:** `portal.dns-cutover.json` الآن يتم إضافة قسم
> `gateway_binding` يشير إلى هذه المصنوعات (المسارات، المحتوى CID، حالة الإثبات، إلخ)
> قالب الرؤوس الحرفي) **y** مرجع `route_plan`
> `gateway.route_plan.json` المزيد من قوالب الرؤوس الرئيسية والتراجع. تشمل هذا
> كتل وكل تذكرة تغيير DG-3 حتى يتمكن المراجعون من مقارنة القيم
> التفاصيل الدقيقة لـ `Sora-Name/Sora-Proof/CSP` والتأكد من خطط الترويج/التراجع
> يتزامن مع حزمة الأدلة دون فتح ملف البناء.

## الخطوة 9 - تشغيل شاشات النشر

يتطلب عنصر خريطة الطريق **DOCS-3c** أدلة مستمرة على أن البوابة والوكيل جربها بنفسك
يتم الاحتفاظ بربطات البوابة بشكل جيد بعد تحريرها. تشغيل الشاشة الموحدة
على الفور بعد الخطوات 7-8 والاتصال ببرامج التحقيق الخاصة بك:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` تحميل ملف التكوين (الإصدار
  `docs/portal/docs/devportal/publishing-monitoring.md` للإسكويما) ذ
  تنفيذ ثلاث عمليات تحقق: تحقيقات مسارات البوابة + التحقق من سياسة CSP/Permissions،
  تحقيقات الوكيل جربها (اختياريًا لنقطة النهاية `/metrics`)، والمتحقق
  ربط البوابة (`cargo xtask soradns-verify-binding`) الذي يتطلب الآن وجودًا
  القيمة المتوقعة من Sora-Content-CID جنبًا إلى جنب مع عمليات التحقق من الاسم المستعار/البيان.
- الأمر النهائي مع عدم وجود صفر عندما يفشل أحد المسبار لـ CI أو وظائف cron أو المشغلين
  يمكن لـ runbook إيقاف إصدار قبل الترويج للاسم المستعار.
- Pasar `--json-out` يكتب استئنافًا لـ JSON unico con estado por target؛ `--evidence-dir`
  انبعاث `summary.json` و`portal.json` و`tryit.json` و`binding.json` و`checksums.sha256` لذلك
  يمكن لمراجعي الإدارة مقارنة النتائج دون إعادة تشغيل الشاشات. أرشيفا
  هذا الدليل باجو `artifacts/sorafs/<tag>/monitoring/` جنبًا إلى جنب مع الحزمة Sigstore y el
  واصف قطع DNS.
- تضمين مخرج الشاشة، التصدير Grafana (`dashboards/grafana/docs_portal.json`)،
  ومعرف أداة التنبيه وبطاقة التحرير حتى يمكن تشغيل SLO DOCS-3c
  com.auditado luego. كتاب اللعب المخصص لمراقبة النشر المباشر
  `docs/portal/docs/devportal/publishing-monitoring.md`.

تتطلب تحقيقات البوابة HTTPS والعثور على عناوين URL الأساسية `http://` على الأقل `allowInsecureHttp`
تم تكوينه في تكوين الشاشة؛ نواصل أهداف الإنتاج/العرض المسرحي عبر TLS ومنفردًا
تمكن من التجاوز لمعاينة اللغات.

أتمتة الشاشة عبر `npm run monitor:publishing` في Buildkite/الكرون مرة واحدة في البوابة
هذا هو الحال في الجسم الحي. الأمر نفسه، قم بإضافة عناوين URL للإنتاج، وقم بتزويد فحوصات الصحة
الاستمرار في استخدام SRE/Docs لجميع الإصدارات.

## الأتمتة مع `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` مغلف لوس باسوس 2-6. إستي:

1. أرشفة `build/` في محدد كرة القطران،
2. قم بالإخراج `car pack`، `manifest build`، `manifest sign`، `manifest verify-signature`،
   ذ `proof verify`,
3. تنفيذ اختياري `manifest submit` (بما في ذلك ربط الاسم المستعار) عند الحصول على بيانات الاعتماد
   Torii، ذ
4. أكتب `artifacts/sorafs/portal.pin.report.json`، اختياري
  `portal.pin.proposal.json`، واصف DNS المقطوع (بعد التقديمات)،
  حزمة ربط البوابة (`portal.gateway.binding.json` بالإضافة إلى كتلة الرؤوس)
  حتى تتمكن أجهزة الحوكمة والشبكات والعمليات من مقارنة حزمة الأدلة
  لا توجد سجلات مراجعة لـ CI.

التكوين `PIN_ALIAS`، `PIN_ALIAS_NAMESPACE`، `PIN_ALIAS_NAME`، ذ (اختياري)
`PIN_ALIAS_PROOF_PATH` قبل استدعاء البرنامج النصي. الولايات المتحدة الأمريكية `--skip-submit` جافة
يدير؛ تم وصف سير العمل في GitHub مرة أخرى عبر الإدخال `perform_submit`.

## الخطوة 8 - المواصفات العامة OpenAPI وحزم SBOM

يتطلب DOCS-7 إنشاء البوابة والمواصفات OpenAPI وبيانات SBOM المنقولة
من خلال تحديد خط الأنابيب بنفس الطريقة. المساعدون الموجودون هم ثلاثة أشخاص:

1. **تجديد المواصفات وتثبيتها.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   قم بوضع علامة الإصدار عبر `--version=<label>` عندما تريد الاحتفاظ بلقطة
   تاريخي (على سبيل المثال `2025-q3`). المساعد يكتب اللقطة en
   `static/openapi/versions/<label>/torii.json`، يرجى الاطلاع على ذلك
   `versions/current`، وقم بتسجيل البيانات التعريفية (SHA-256، حالة البيان، و
   تم تحديث الطابع الزمني) في `static/openapi/versions.json`. بوابة المطورين
   هذا مؤشر على أن لوحات Swagger/RapiDoc يمكنها تقديم محدد الإصدار
   وإظهار الملخص/الشركة المرتبطة عبر الإنترنت. حذف `--version` احتفظ بعلامات التبويب
   الإصدار السابق سليم كما تم تحديثه منفردًا للنقاط `current` + `latest`.

   يلتقط البيان ملخص SHA-256/BLAKE3 حتى تتمكن البوابة من الإمساك به
   الرؤوس `Sora-Proof` لـ `/reference/torii-swagger`.

2. **Emite SBOMs CycloneDX.** خط أنابيب إطلاق سراح SBOMs القائم على النظام
   سيجون `docs/source/sorafs_release_pipeline_plan.md`. حافظ على الخلاص
   جنبًا إلى جنب مع قطع البناء:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **إكمال كل حمولة في سيارة.**

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

   اتبع نفس الخطوات من `manifest build` / `manifest sign` إلى الموقع الرئيسي،
   ضبط الاسم المستعار للمنتج (على سبيل المثال، `docs-openapi.sora` للمواصفات
   `docs-sbom.sora` لحزمة SBOM الثابتة). Mantener الاسم المستعار distintos mantiene SoraDNS،
   GARs وتذاكر التراجع عن الحمولة تمامًا.

4. **إرسال وربط.** إعادة استخدام المصدر الموجود والحزمة Sigstore، فقط للتسجيل
   مجموعة الاسم المستعار في قائمة المراجعة التي تم إصدارها حتى يتمكن المستمعون من تحديد اسم سورا
   خريطة لكي هضم البيان.

أرشفة بيانات المواصفات/SBOM جنبًا إلى جنب مع إنشاء البوابة للتأكد من كل تذكرة
قم بتحرير مجموعة القطع الأثرية الكاملة دون إعادة تشغيل الباكر.

### مساعد الأتمتة (البرنامج النصي CI/الحزمة)

`./ci/package_docs_portal_sorafs.sh` تدوين الخطوات 1-8 لبند خريطة الطريق
**DOCS-7** يمكنك اللعب بأمر واحد فقط. المساعد:

- تنفيذ الإعداد المطلوب للبوابة (`npm ci`، مزامنة OpenAPI/norito، اختبارات عناصر واجهة المستخدم)؛
- قم بإصدار سيارات وبيانات البوابة الإلكترونية، OpenAPI وSBOM عبر `sorafs_cli`؛
- تنفيذ اختياري `sorafs_cli proof verify` (`--proof`) والشركة Sigstore
  (`--sign`، `--sigstore-provider`، `--sigstore-audience`)؛
- deja todos los artefactos bajo `artifacts/devportal/sorafs/<timestamp>/` y
  قم بكتابة `package_summary.json` حتى تتمكن من تثبيت CI/tooling على الحزمة؛ ذ
- قم بتحديث `artifacts/devportal/sorafs/latest` لبدء التشغيل الأحدث.

مثال (خط الأنابيب مكتمل مع Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

أعلام الكونسيرج:

- `--out <dir>` - تجاوز جذر المصنوعات اليدوية (الحفاظ الافتراضي على السجاد مع الطابع الزمني).
- `--skip-build` - إعادة استخدام `docs/portal/build` الموجود (باستخدام CI لا يمكن
  إعادة بناء المرايا دون اتصال بالإنترنت).
- `--skip-sync-openapi` - حذف `npm run sync-openapi` عند `cargo xtask openapi`
  لا يمكنك الحصول على صناديق.io.
- `--skip-sbom` - قم بالاتصال بـ `syft` عند عدم تثبيت الملف الثنائي (يتم طباعة البرنامج النصي
  إعلان في مكانك).
- `--proof` - قم بتشغيل `sorafs_cli proof verify` لكل سيارة/بيان. الحمولات دي
  تتطلب الأرشيفات المتعددة دعم مخطط القطعة في CLI، حتى يتم وضع هذه العلامة
  بدون التثبيت إذا تم اكتشاف أخطاء `plan chunk count` والتحقق يدويًا عند
  llegue el gate المنبع.
- `--sign` - الاستدعاء `sorafs_cli manifest sign`. إثبات رمز مميز
  `SIGSTORE_ID_TOKEN` (o `--sigstore-token-env`) أو دع CLI تحصل على الاستخدام
  `--sigstore-provider/--sigstore-audience`.عندما تحسد المصنوعات اليدوية من الولايات المتحدة الأمريكية `docs/portal/scripts/sorafs-pin-release.sh`.
الآن قم بتغليف البوابة الإلكترونية، OpenAPI والحمولات SBOM، وتأكيد كل بيان، وتسجيل بيانات التعريف
الأصول الإضافية في `portal.additional_assets.json`. المساعد يدرك نفس المقابض الاختيارية
يتم استخدام وحدة حزم CI بالإضافة إلى المفاتيح الجديدة `--openapi-*` و`--portal-sbom-*` وy
`--openapi-sbom-*` لتعيين مجموعات من الأسماء المستعارة للمنتج، وتجاوز مصدر SBOM عبر
`--openapi-sbom-source`، الحمولات الصافية المحذوفة (`--skip-openapi`/`--skip-sbom`)،
قم بالنقر على ملف ثنائي `syft` غير افتراضي مع `--syft-bin`.

يعرض البرنامج النصي كل أمر يتم تنفيذه؛ انسخ السجل في تذكرة الإصدار
جنبًا إلى جنب مع `package_summary.json` حتى تتمكن المراجع من مقارنة ملخصات CAR وبيانات التعريف
الخطة، وتجزئة الحزمة Sigstore بدون مراجعة الصدفة المخصصة.

## الخطوة 9 - التحقق من البوابة + SoraDNS

قبل الإعلان عن عملية القطع، تأكد من ظهور الاسم المستعار الجديد عبر SoraDNS والبوابات
إنغرابان بروباس فريسكا:

1. **قم بتشغيل بوابة المسبار.** `ci/check_sorafs_gateway_probe.sh` يتم تشغيله
   `cargo xtask sorafs-gateway-probe` ضد التركيبات التجريبية أون
   `fixtures/sorafs_gateway/probe_demo/`. للاستمتاع بالواقع، قم بالتحقيق في هدف اسم المضيف:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   المسبار فك التشفير `Sora-Name`, `Sora-Proof`, و`Sora-Proof-Status` ثانية
   `docs/source/sorafs_alias_policy.md` وينتهي عند ملخص البيان،
   يتم حذف TTLs أو روابط GAR.

   لإجراء فحوصات مفاجئة خفيفة الوزن (على سبيل المثال، عند ربط الحزمة فقط
   تم التغيير)، قم بتشغيل `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   يقوم المساعد بالتحقق من صحة حزمة الربط التي تم التقاطها وهو سهل الإصدار
   التذاكر التي تحتاج فقط إلى تأكيد ملزم بدلاً من تدريبات التحقيق الكاملة.

2. **التقاط أدلة التدريبات.** لتدريبات المشغل أو محاكاة PagerDuty، قم بإرفاقها
   المسبار مع البرامج النصية/القياس عن بعد/run_sorafs_gateway_probe.sh --السيناريو
   طرح devportal -- ...`. المجمع يحرس الرؤوس/السجلات
   `artifacts/sorafs_gateway_probe/<stamp>/`، التحديث `ops/drill-log.md`، نعم
   (اختياريًا) يفصل خطافات التراجع أو الحمولات النافعة PagerDuty. إستابليس
   `--host docs.sora` للتحقق من مسار SoraDNS في مكان التشفير الثابت لعنوان IP.

3. **التحقق من روابط DNS.** عندما تنشر الإدارة إثبات الاسم المستعار، قم بتسجيل ملف GAR
   تم الرجوع إلى المسبار (`--gar`) وإضافته إلى دليل الإصدار. هم أصحاب الحل
   يمكنك تحرير نفس الإدخال عبر `tools/soradns-resolver` لضمان الإدخالات في ذاكرة التخزين المؤقت
   احترام البيان الجديد. قبل إضافة JSON، قم بالتشغيل
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   حتى يتم تحديد الخريطة للمضيف وبيانات التعريف وبيانات القياس عن بعد
   صالح حاليا. يمكن للمساعد إصدار استئناف `--json-out` جنبًا إلى جنب مع GAR الثابت لهم
   تحتوي المراجعات على أدلة يمكن التحقق منها دون فتح الثنائي.
  عندما يقوم بتنقيح GAR جديد، تفضل
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (قم بعرض `--manifest-cid <cid>` فقط عندما يكون ملف البيان غير متوفر). المساعد
  الآن اشتقاق CID **y** ملخص BLAKE3 مباشرة من بيان JSON، قم بقص المساحات باللون الأبيض،
  علامات إزالة التكرارات `--telemetry-label` المتكررة، وترتيب العلامات، وإصدار القوالب بشكل افتراضي
  CSP/HSTS/Permissions-Policy قبل كتابة JSON بحيث يتم تحديد الحمولة النافعة
  بما في ذلك عندما يلتقط المشغلون إرشادات من أغطية مختلفة.

4. **ملاحظة مقاييس الاسم المستعار.** Manten `torii_sorafs_alias_cache_refresh_duration_ms`
   y `torii_sorafs_gateway_refusals_total{profile="docs"}` والشاشة أمام المسبار
   كور؛ سلسلة ambas estan graphicadas en `dashboards/grafana/docs_portal.json`.

## الخطوة 10 - مراقبة وحزمة الأدلة

- **لوحات المعلومات.** تصدير `dashboards/grafana/docs_portal.json` (SLOs للبوابة)،
  `dashboards/grafana/sorafs_gateway_observability.json` (زمن الوصول للبوابة +
  صحة الإثبات)، `dashboards/grafana/sorafs_fetch_observability.json`
  (salud del Orchestra) لكل إصدار. ملحق الصادرات JSON آل تذكرة دي
  قم بتحريره حتى تتمكن المراجعات من إعادة إنتاج الاستعلامات الخاصة بـ Prometheus.
- **ملفات المسبار.** حفظ `artifacts/sorafs_gateway_probe/<stamp>/` في git-annex
  يا دلو الأدلة. يتضمن السيرة الذاتية للمسبار والرؤوس والحمولة النافعة PagerDuty
  تم التقاطه بواسطة سيناريو القياس عن بعد.
- **إصدار الحزمة.** حفظ السيرة الذاتية لـ CAR للبوابة/SBOM/OpenAPI، وحزم البيانات،
  الشركات Sigstore و`portal.pin.report.json` وسجلات اختبار Try-It وتقارير التحقق من الارتباط
  احتفظ بسجادة ذات طابع زمني (على سبيل المثال، `artifacts/sorafs/devportal/20260212T1103Z/`).
- **سجل الحفر.** عندما تكون المسابير جزءًا من الحفر، قم بذلك
  `scripts/telemetry/run_sorafs_gateway_probe.sh` يجمع المدخلات مع `ops/drill-log.md`
  لكي يثبت هذا الأمر أنه يلبي متطلبات SNNet-5.
- **روابط التذكرة.** راجع معرفات لوحة Grafana أو تصدير PNG الملحقات على التذكرة
  من خلال التغيير، جنبًا إلى جنب مع مسار تقرير التحقيق، حتى تتمكن المراجعات من الانتقال إلى SLOs
  دون الوصول إلى قذيفة.

## الخطوة 11 - أداة جلب متعددة المصادر ولوحة النتائج

نشر في SoraFS الآن يتطلب أدلة جلب متعددة المصادر (DOCS-7/SF-6)
جنبًا إلى جنب مع اختبارات DNS/البوابة السابقة. بعد ظهور البيان:

1. **قم بتشغيل `sorafs_fetch` ضد البيان المباشر.** استخدم نفس نتاجات الخطة/البيان
   يتم إنتاجه في الخطوات 2-3 بالإضافة إلى شهادات اعتماد البوابة الصادرة لكل مورد. الثبات
   كل ما في الأمر هو أن المستمعين يمكنهم إعادة إنتاج قرار المنسق:

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
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

   - ابحث أولاً عن إعلانات الموردين المرجعية حسب البيان (على سبيل المثال
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     والخطوات عبر `--provider-advert name=path` حتى تتمكن لوحة النتائج من تقييم النوافذ
     القدرة على شكل محدد. الولايات المتحدة الأمريكية
     `--allow-implicit-provider-metadata` **منفردًا** عند إعادة إنتاج التركيبات في CI؛ تدريبات لوس
     يجب أن يعتمد الإنتاج على الإعلانات التجارية التي ستتوافق مع الدبوس.
   - عندما يشير البيان إلى مناطق إضافية، كرر الأمر مع مجموعات
     الموردون المراسلون لكي تتمكن كل ذاكرة تخزين مؤقت/اسم مستعار من الحصول على قطعة أثرية مرتبطة.

2. ** أرشيفا لاس ساليداس. ** جواردا `scoreboard.json`،
   `providers.ndjson`، `fetch.json`، و`chunk_receipts.ndjson` خلف سجادة الأدلة
   الافراج. تلتقط هذه المحفوظات وزن النظراء وميزانية إعادة المحاولة ووقت استجابة EWMA وما إلى ذلك
   إيصالات بسبب أن حزمة الإدارة يجب الاحتفاظ بها لـ SF-7.

3. **تحديث القياس عن بعد.** استيراد نتائج الجلب على لوحة القيادة **SoraFS Fetch
   إمكانية الملاحظة** (`dashboards/grafana/sorafs_fetch_observability.json`)،
   ملاحظة `torii_sorafs_fetch_duration_ms`/`_failures_total` وألواح النطاق من قبل المورّد
   بارا الشذوذ. قم بلصق لقطات اللوحة Grafana في تذكرة الإصدار مع المسار
   لوحة النتائج.

4. **قم بتفعيل قواعد التنبيه.** قم بتشغيل `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   للتحقق من حزمة التنبيهات Prometheus قبل إغلاق الإصدار. أضف الخلاصة
   أداة الترويج للتذكرة لتأكيد مراجع DOCS-7 على تنبيهات التوقف
   أسطول siguen مزود بطيء.

5. **كابل CI.** يحافظ سير العمل على دبوس البوابة على خطوة `sorafs_fetch` من الإدخال
   `perform_fetch_probe`; تأهيل لتنفيذ عمليات التمثيل المسرحي/الإنتاج حتى تتمكن من إثبات ذلك
   قم بإحضار المنتج جنبًا إلى جنب مع حزمة البيانات بدون دليل التدخل. يمكن إجراء التدريبات المحلية
   إعادة استخدام البرنامج النصي نفسه لتصدير الرموز المميزة للبوابة وتثبيت `PIN_FETCH_PROVIDERS`
   قائمة الموردين منفصلة بفواصل.

## الترويج وقابلية المراقبة والتراجع

1. **الترويج:** احتفظ بالاسم المستعار المنفصل للعرض والإنتاج. إعادة التنفيذ الترويجي
   `manifest submit` بنفس البيان/الحزمة، التغيير
   `--alias-namespace/--alias-name` لتطوير الاسم المستعار للإنتاج. هذه هي الحياة
   قم بإعادة البناء أو التثبيت مرة أخرى عندما يتم ضبط ضمان الجودة على دبوس التدريج.
2. **المراقبة:** استيراد لوحة معلومات التسجيل
   (`docs/source/grafana_sorafs_pin_registry.json`) هي المسابير الخاصة بالبوابة
   (الإصدار `docs/portal/docs/devportal/observability.md`). تنبيه في الانجراف دي المجموع الاختباري،
   تحقيقات فشل أو صور إعادة محاولة الإثبات.
3. **التراجع:** للرجوع، وإعادة النظر في البيان السابق (أو إزالة الاسم المستعار الفعلي) باستخدام
   `sorafs_cli manifest submit --alias ... --retire`. حافظ على الحزمة الأخيرة
   أعلم أنه جيد واستئناف CAR حتى تتمكن تجارب التراجع من المحاولة مرة أخرى
   يتم تدوير سجلات CI.

## مصنع سير العمل CI

كم الحد الأدنى، خط الأنابيب الخاص بك:

1. البناء + الوبر (`npm ci`، `npm run build`، توليد المجموع الاختباري).
2. قم بتجميع (`car pack`) وبيانات الكمبيوتر.
3. تثبيت استخدام الرمز المميز OIDC للمهمة (`manifest sign`).
4. المصنوعات اليدوية الفرعية (السيارة، البيان، الحزمة، الخطة، الملخصات) للمستمعين.
5. Enviar آل دبوس التسجيل:
   - سحب الطلبات -> `docs-preview.sora`.
   - العلامات / الفروع المحمية -> الترويج للاسم المستعار للإنتاج.
6. قم بإخراج المجسات + بوابات التحقق من الإثبات قبل الانتهاء.

`.github/workflows/docs-portal-sorafs-pin.yml` يتواصل مع جميع هذه الخطوات لإصدارات الأدلة.
سير العمل:

- بناء / اختبار البوابة الإلكترونية،
- empaqueta el build عبر `scripts/sorafs-pin-release.sh`،
- تأكيد/التحقق من حزمة البيانات لاستخدام GitHub OIDC،
- sube el CAR/manificto/bundle/plan/resumenes as a Artifacts، y
- (اختياريًا) أرسل البيان + الاسم المستعار الملزم عند أسرار القش.

قم بتكوين أسرار/متغيرات المستودع التالية قبل تغيير الوظيفة:| الاسم | اقتراح |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | المضيف Torii الذي يعرض `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | معرف عصر التسجيل مع التقديمات. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | التلقائية الثابتة لإرسال البيان. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | تم حفظ الاسم المستعار في البيان cuando `perform_submit` es `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | حزمة إثبات الاسم المستعار المشفر في Base64 (اختياري؛ حذف للحصول على الاسم المستعار الملزم). |
| `DOCS_ANALYTICS_*` | يتم إعادة استخدام نقاط النهاية للتحليلات/التحقق من وجودها من خلال سير عمل آخر. |

تغيير سير العمل عبر واجهة المستخدم الخاصة بالإجراءات:

1. قم بإثبات `alias_label` (على سبيل المثال، `docs.sora.link`)، `proposal_alias` اختياريًا،
   وتجاوز اختياري لـ `release_tag`.
2. Deja `perform_submit` بدون علامة تجارية لإنشاء قطع أثرية بدون Torii
   (استخدام للتشغيل الجاف) أو التمكن من النشر مباشرة على الاسم المستعار الذي تم تكوينه.

`docs/source/sorafs_ci_templates.md` أحد المستندات المساعدة لـ CI العامة
المشاريع فورًا من هذا المستودع، لكن يجب أن يكون سير عمل البوابة هو الخيار المفضل
الفقرة النشرات ديل ديا ضياء.

## قائمة المراجعة

- [ ] `npm run build`، `npm run test:*`، و`npm run check:links` موجود في اللون الأخضر.
- [ ] تم التقاط `build/checksums.sha256` و`build/release.json` من خلال قطع أثرية.
- [ ] CAR، خطة، إعلان، واستئناف إنشاء `artifacts/`.
- [ ] الحزمة Sigstore + شركة منفصلة لسجلات المخازن.
- [ ] `portal.manifest.submit.summary.json` و `portal.manifest.submit.response.json`
      capturados cuando hay التقديمات.
- [ ] `portal.pin.report.json` (واختياري `portal.pin.proposal.json`)
      تم أرشفته جنبًا إلى جنب مع CAR/manifyingto المصنوعات اليدوية.
- [ ] سجلات أرشيفات `proof verify` و`manifest verify-signature`.
- [ ] لوحات معلومات Grafana تم تحديثها + تحقيقات Try-It خارجة.
- [ ] ملاحظات التراجع (معرف البيان السابق + ملخص الاسم المستعار) الملحقات الإضافية
      تذكرة الافراج.