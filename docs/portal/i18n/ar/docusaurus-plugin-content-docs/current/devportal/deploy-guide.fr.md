---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## عرض الفرقة

يقوم كتاب اللعب هذا بتحويل عناصر خريطة الطريق **DOCS-7** (المنشور SoraFS) و **DOCS-8**
(أتمتة دبوس CI/CD) وهو إجراء قابل للتنفيذ لتطوير البوابة.
تغطي مرحلة البناء/الوبر، التغليف SoraFS، توقيع البيانات مع Sigstore،
ترقية الأسماء المستعارة والتحقق وتدريبات التراجع لكل معاينة وإصدار
لذا فهي قابلة للتكرار وقابلة للتدقيق.

يفترض التدفق أنك حصلت على الملف الثنائي `sorafs_cli` (تجميع مع `--features cli`)، والوصول إلى
نقطة نهاية Torii مع أذونات تسجيل الدخول وبيانات الاعتماد OIDC لـ Sigstore.
تخزين الأسرار لفترة طويلة (`IROHA_PRIVATE_KEY`، `SIGSTORE_ID_TOKEN`، الرموز المميزة Torii) في
صندوق الاقتراع CI؛ يمكن أن يتم شحن عمليات التنفيذ المحلية عبر صادرات du Shell.

## المتطلبات الأساسية

- العقدة 18.18+ مع `npm` أو `pnpm`.
- `sorafs_cli` عبر `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- عنوان URL Torii الذي يعرض `/v1/sorafs/*` بالإضافة إلى حساب/cle خاص بأداة التفويض التي يمكن أن تكون كذلك
  البيانات والاسم المستعار.
- Emmeteur OIDC (GitHub Actions، GitLab، هوية عبء العمل، وما إلى ذلك) من أجل الحصول على `SIGSTORE_ID_TOKEN`.
- الخيار: `examples/sorafs_cli_quickstart.sh` من أجل التشغيل الجاف وآخرون
  `docs/source/sorafs_ci_templates.md` لدعم سير عمل GitHub/GitLab.
- تكوين متغيرات OAuth جربها (`DOCS_OAUTH_*`) وقم بتنفيذها
  [قائمة التحقق من تعزيز الأمان](./security-hardening.md) قبل ترقية البناء
  خارج المختبر. يتم الحفاظ على صدى إنشاء الباب عند استمرار هذه المتغيرات
  أو عند استخدام مقابض TTL/فرز الحواجز المفروضة؛ Exportez
  `DOCS_OAUTH_ALLOW_INSECURE=1` فريد من نوعه لمعاينة اللغات المحلية. جوينييز
  اختبار القلم في تذكرة الإصدار.

## Etape 0 - برنامج التقاط حزمة الوكيل جربه

قبل الترويج لمعاينة عبر Netlify أو البوابة، اكتشف مصادر الوكيل جربها
ولخص البيان OpenAPI في حزمة محددة:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` نسخ المساعدين الوكيل/المسبار/الاستعادة، التحقق من التوقيع
OpenAPI وأكتب `release.json` بالإضافة إلى `checksums.sha256`. Joignez ce package au Ticket de
بوابة الترويج Netlify/SoraFS حتى يتمكن المراجعون من تجديد المصادر الدقيقة
والتلميحات حول الهدف Torii بدون إعادة البناء. يتم تسجيل الحزمة أيضًا إذا كان حاملوها
تم توفيره بواسطة العميل النشط (`allow_client_auth`) لحماية خطة الطرح
وقواعد CSP على مراحل.

## الشريط 1 - بناء وبطانة الباب

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

`npm run build` تنفيذ الإجراء التلقائي `scripts/write-checksums.mjs`، المنتج:

- `build/checksums.sha256` - بيان SHA256 لـ `sha256sum -c`.
- `build/release.json` - إصلاحات البيانات الوصفية (`tag`، `generated_at`، `source`) في كل سيارة/بيان.

أرشفة هذه الملفات باستخدام السيرة الذاتية CAR حتى يتمكن المراجعون من مقارنة القطع الأثرية
معاينة بلا إعادة بناء.

## الشريط 2 - قم بتعبئة إحصائيات الأصول

قم بتنفيذ أداة التعبئة CAR ضد ذخيرة الفرز Docusaurus. مثال على ذلك: الكتابة
القطع الأثرية سو `artifacts/devportal/`.

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

يلتقط استئناف JSON حسابات القطع والملخصات وتلميحات تخطيط الإثبات
que `manifest build` et les Dashboards CI reutilisent plus tard.

## Etape 2b - Empaqueter lesرفاق OpenAPI et SBOM

يتطلب DOCS-7 نشر موقع البوابة واللقطة OpenAPI وحمولات SBOM
كبيانات مميزة حتى تتمكن البوابات من جذب الرؤوس
`Sora-Proof`/`Sora-Content-CID` لكل قطعة أثرية. مساعد الافراج
(`scripts/sorafs-pin-release.sh`) قم بتغطية المرجع OpenAPI
(`static/openapi/`) وتصدر SBOMs عبر `syft` في السيارات المنفصلة
`openapi.*`/`*-sbom.*` وقم بتسجيل البيانات التعريفية في
`artifacts/sorafs/portal.additional_assets.json`. إن التدفق مانويل,
كرر الأشرطة 2-4 لكل حمولة مع البادئات الخاصة والبيانات الوصفية
(على سبيل المثال `--car-out "$OUT"/openapi.car` plus
`--metadata alias_label=docs.sora.link/openapi`). قم بتسجيل كل زوج من البيان/الاسم المستعار
على Torii (الموقع، OpenAPI، بوابة SBOM، SBOM OpenAPI) قبل حجب DNS لذلك
تخدم البوابة الخدمات المقدمة لجميع القطع الأثرية المنشورة.

## الشريط 3 – بناء البيان

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

اضبط علامات سياسة الدبوس على نافذة التحرير الخاصة بك (على سبيل المثال، `--pin-storage-class
حار ` صب ليه الكناري). يعد متغير JSON خيارًا أكثر عملية لمراجعة الكود.

## الشريط 4 - التوقيع مع Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

تقوم الحزمة بتسجيل ملخص البيان وملخصات القطع وتجزئة BLAKE3 للرمز المميز
OIDC بدون استمرار في JWT. Gardez le Bundle et la Signature Detachee؛ الترقيات دي
يمكن أن يؤدي الإنتاج إلى إعادة استخدام الميمات الأثرية بدلاً من إعادة التوقيع. عمليات الإعدام المحلية
يمكن استبدال موفر الأعلام على أساس `--identity-token-env` (أو تحديد `SIGSTORE_ID_TOKEN`
) عند وجود مساعد OIDC خارجي يصدر الرمز المميز.

## Etape 5 - سجل Soumettre au pin

ضع علامة البيان (وخطة القطع) على Torii. Demandez toujours un استئناف صب
أن النتيجة المدخلة/الاسم المستعار قابلة للتدقيق.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority <katakana-i105-account-id> \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

عند بدء تشغيل المعاينة أو الكناري (`docs-preview.sora`)، كرر الإرسال
مع اسم مستعار فريد من نوعه لكي تتمكن QA من التحقق من المحتوى قبل الترويج للإنتاج.

تتطلب الأسماء المستعارة للتجليد ثلاثة أبطال: `--alias-namespace` و`--alias-name` و`--alias-proof`.
حوكمة المنتج لمجموعة الأدلة (base64 أو bytes Norito) عند طلب الاسم المستعار
تمت الموافقة عليه؛ قم بتخزين أسرار CI وكشفها كملف قبل الاستدعاء
`manifest submit`. Laissez les flags d'alias vides quand vous voulez seulement pinner
البيان بدون لمس DNS.

## الشريط 5ب - إنشاء اقتراح للحوكمة

كل بيان يجب أن يسافر مع اقتراح مقدم للبرلمان ليتمكن كل المواطنين
لذلك يمكنك تقديم التغيير دون استغلال امتيازات بيانات الاعتماد. بعد ذلك
إرسال/توقيع الأشرطة، لانسز:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` التقاط التعليمات الكنسي `RegisterPinManifest`،
ملخص القطع والسياسة ومؤشر الاسم المستعار. انضم إلى بطاقة الحكم أو إلى
بوابة البرلمان حتى يتمكن المندوبون من مقارنة الحمولة دون إعادة بناءها.
كأمر لا تضغط على مفتاح التشغيل Torii، يمكن لأي مواطن إعادة ضبطه
موضع الاقتراح.

## الشريط 6 - التحقق من البراهين والقياس عن بعد

بعد الدبوس، قم بتنفيذ خطوات التحقق المحددة:

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

- المراقبة `torii_sorafs_gateway_refusals_total` وآخرون
  `torii_sorafs_replication_sla_total{outcome="missed"}` من أجل الحالات الشاذة.
- قم بتنفيذ `npm run probe:portal` لتشغيل الوكيل Try-It وتسجيل الامتيازات
  contre le contenu fraichement pinne.
- قم بالتقاط أدلة المراقبة التي يتم تحديدها داخل الكاميرا
  [النشر والمراقبة](./publishing-monitoring.md) لبوابة المراقبة
  DOCS-3c يرضي أوراق النشر. المساعد يقبل الصيانة
  مداخل إضافية `bindings` (الموقع، OpenAPI، بوابة SBOM، SBOM OpenAPI) والتنفيذ
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` على الكابل الساخن عبر خيار الحماية
  `hostname`. قم بكتابة الاستدعاء من خلال استئناف JSON فريد وحزمة الأدلة
  (`portal.json`، `tryit.json`، `binding.json`، `checksums.sha256`) ضمن المرجع
  دي الإصدار:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## الشريط 6أ - مخطط شهادات البوابة

استخرج خطة SAN/challenge TLS قبل إنشاء حزم GAR لتجهيز البوابة وما إلى ذلك
يقوم معتمدو DNS بفحص الأدلة. يساعد الجديد في إعادة إدخال المدخلات DG-3
قم بتعداد المضيفين لأحرف البدل الأساسية والمضيف الجميل لـ SANs والتسميات DNS-01 وما إلى ذلك
التحديات التي توصي بها ACME:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

قم بتثبيت JSON مع حزمة الإصدار (أو قم بتحميلها مع تذكرة التغيير) من أجل
يمكن للمشغلين ربط قيم SAN في التكوين
`torii.sorafs_gateway.acme` de Torii وما يمكن للمراجعين GAR تأكيده
تعيينات canonique/pretty sans إعادة تنفيذ اشتقاقات الساخنة. إضافة الحجج
`--name` إضافات لكل لاحقة يتم الترويج لها في إصدار meme.

## Etape 6b - Deriver les Mappings d'hotes canoniques

قبل إنشاء حمولات GAR، قم بتسجيل التعيين المحدد لكل اسم مستعار.
`cargo xtask soradns-hosts` hashe chaque `--name` en son label canonique
(`<base32>.gw.sora.id`)، وحذف حرف البدل المطلوب (`*.gw.sora.id`) واشتقاق المضيف الجميل
(`<alias>.gw.sora.name`). استمر في عملية الإطلاق داخل العناصر التي سيتم تحريرها حتى تتمكن من ذلك
المراجعين DG-3 يمكن مقارنة رسم الخرائط بمتطلبات GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

استخدم `--verify-host-patterns <file>` لتكرار Vite lorsqu'un GAR أو JSON de
بوابة الربط Omet un des hosts requis. المساعد يقبل ملفات التحقق الإضافية،
ما الذي يسهل الوبر من قالب GAR و`portal.gateway.binding.json` agrafe dans
استدعاء ميمي:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```انضم إلى استئناف JSON وسجل التحقق من خلال تذكرة تغيير DNS/البوابة من أجل ذلك
يمكن للمدققين تأكيد المضيفين التقليديين وأحرف البدل والجميلة دون إعادة تنفيذهم
مخطوطات. أعد تنفيذ الأمر عندما يتم إضافة الاسم المستعار الجديد إلى الحزمة من أجل ذلك
تحديثات GARheritent de la meme الأدلة.

## الشريط 7 - إنشاء واصف قطع DNS

يتطلب إنتاج القطع حزمة تغيير قابلة للتدقيق. بعد الاستسلام
reussie (اسم مستعار ملزم) ، المساعد emet
`artifacts/sorafs/portal.dns-cutover.json`، الآسر:

- البيانات التعريفية للأسماء المستعارة الملزمة (مساحة الاسم/الاسم/الدليل، ملخص البيان، URL Torii،
  عصر soumis، autorite)؛
- سياق الإصدار (العلامة، التسمية المستعارة، بيان chemins/CAR، خطة القطع، الحزمة Sigstore)؛
- مؤشرات التحقق (مسبار الأمر، الاسم المستعار + نقطة النهاية Torii)؛ وآخرون
- خيارات التحكم في التغيير (تذكرة الهوية، قطع النوافذ، عمليات الاتصال،
  اسم المضيف/إنتاج المنطقة)؛
- البيانات التعريفية للترويج للطريق المشتق من الرأس `Sora-Route-Binding`
  (المضيف canonique/CID، chemins de header + ملزمة، أوامر التحقق)، أضمن لك ذلك
  الترويج لـ GAR وتدريبات الارتداد التي تشير إلى أدلة ميمي؛
- القطع الأثرية لأنواع خطط المسار (`gateway.route_plan.json`،
  قوالب خيارات التراجع عن الرؤوس والرؤوس) لتذاكر التغيير وما إلى ذلك
  خطافات الوبر CI يمكنها التحقق من أن كل حزمة DG-3 تشير إلى خطط
  قواعد الترويج/التراجع قبل الموافقة؛
- خيار إلغاء صلاحية البيانات الوصفية لذاكرة التخزين المؤقت (نقطة نهاية التطهير، المصادقة المتغيرة، الحمولة JSON،
  ومثال الأمر `curl`)؛ وآخرون
- تلميحات حول التراجع عن الوصف السابق (علامة الإصدار وخلاصة البيان)
  لكي تلتقط التذاكر طريقًا احتياطيًا محددًا.

عندما يتطلب الإصدار عمليات تطهير لذاكرة التخزين المؤقت، قم بإنشاء خطة أساسية باستخدام الواصف:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

قم بتوصيل `portal.cache_plan.json` إلى حزمة DG-3 لتمكين المشغلين من الوصول إلى المضيفين/المسارات
 المحددات (وتلميحات المصادقة المقابلة) عندما يتم إصدار الطلبات `PURGE`.
يمكن أن يشير قسم خيار ذاكرة التخزين المؤقت للواصف إلى الملف مباشرة، بشكل مباشر
يتطرق المراجعون إلى عمليات التطهير الدقيقة لنقاط النهاية أثناء عملية القطع.

كل حزمة DG-3 تحتاج أيضًا إلى ترويج قائمة التحقق + التراجع. جينيريز لو عبر
`cargo xtask soradns-route-plan` لكي يتمكن المراجعون من تتبع الخطوات الدقيقة
الاختبار المبدئي والقطع والتراجع الاسم المستعار:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

يلتقط Le `gateway.route_plan.json` emis المضيفين canoniques/جميلة، قمم التحقق من الصحة
على طول الشريط، يتم تحديث ربط GAR، وتطهير ذاكرة التخزين المؤقت، وإجراءات التراجع. Joignez-le aux
المصنوعات اليدوية GAR/التجليد/القطع قبل إضافة تذكرة التغيير حتى تتمكن العمليات من
تكرار وتصحيح الميمات والأشرطة النصية.

`scripts/generate-dns-cutover-plan.mjs` يقوم بتزويد هذا الواصف ويتم تنفيذه تلقائيًا بعد ذلك
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

قم بفحص خيارات البيانات الوصفية عبر متغيرات البيئة قبل تنفيذ مساعد الدبوس:

| متغير | لكن |
| --- | --- |
| `DNS_CHANGE_TICKET` | معرف مخزون التذكرة في الواصف. |
| `DNS_CUTOVER_WINDOW` | نافذة القطع ISO8601 (على سبيل المثال: `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`، `DNS_ZONE` | إنتاج اسم المضيف + سلطة المنطقة. |
| `DNS_OPS_CONTACT` | الاسم المستعار عند الطلب أو الاتصال بـ d'escalade. |
| `DNS_CACHE_PURGE_ENDPOINT` | نقطة النهاية تقوم بتطهير ذاكرة التخزين المؤقت وتسجيلها في الواصف. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var contenant le token purge (الافتراضي: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Chemin ver le descripteur precedent لاستعادة البيانات الوصفية. |

قم بمشاركة JSON في مراجعة تغيير DNS حتى يتمكن المعتمدون من التحقق من الملخصات
البيانات والروابط الاسمية والأوامر التي يتم التحقيق فيها دون حرق سجلات CI. أعلام ليه CLI
`--dns-change-ticket`، `--dns-cutover-window`، `--dns-hostname`،
`--dns-zone`، `--ops-contact`، `--cache-purge-endpoint`،
`--cache-purge-auth-env`، و`--previous-dns-plan` يوفران تجاوزات الميمات
عندما يكون مساعد الدوران خارج CI.

## الشريط 8 - حذف ملف المنطقة من المحلل (اختياري)

عندما تستمر نافذة الإنتاج المقطوعة، قد ينطلق نص الإصدار
يتم مسح ملف المنطقة SNS وحل المقتطف تلقائيًا. مرر ليه
يسجل رغبات DNS والبيانات الوصفية عبر متغيرات البيئة أو خيارات CLI؛ لو المساعد
اتصل `scripts/sns_zonefile_skeleton.py` فورًا بعد إنشاء الواصف.
احصل على أقل قيمة من A/AAAA/CNAME وGAR (BLAKE3-256 من علامة الحمولة GAR).
إذا كانت المنطقة/اسم المضيف معروفة و`--dns-zonefile-out` مفقودة، فسيتم كتابة المساعدة فيها
`artifacts/sns/zonefiles/<zone>/<hostname>.json` واستبداله
`ops/soradns/static_zones.<hostname>.json` هو محلل المقتطفات.

| المتغير / العلم | لكن |
| --- | --- |
| `DNS_ZONEFILE_OUT`، `--dns-zonefile-out` | نوع ملف المنطقة squelette. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`، `--dns-zonefile-resolver-snippet` | محلل المقتطف (الافتراضي: `ops/soradns/static_zones.<hostname>.json` si omis). |
| `DNS_ZONEFILE_TTL`، `--dns-zonefile-ttl` | تسجيلات TTL applique aux (الافتراضي: 600 ثانية). |
| `DNS_ZONEFILE_IPV4`، `--dns-zonefile-ipv4` | عناوين IPv4 (بيئة منفصلة حسب الخصائص أو علامة CLI قابلة للتكرار). |
| `DNS_ZONEFILE_IPV6`، `--dns-zonefile-ipv6` | عناوين IPv6. |
| `DNS_ZONEFILE_CNAME`، `--dns-zonefile-cname` | خيار CNAME المستهدف. |
| `DNS_ZONEFILE_SPKI`، `--dns-zonefile-spki-pin` | دبابيس SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`، `--dns-zonefile-txt` | مقبلات TXT addnelles (`key=value`). |
| `DNS_ZONEFILE_VERSION`، `--dns-zonefile-version` | تجاوز تسمية الإصدار Zonefile حساب. |
| `DNS_ZONEFILE_EFFECTIVE_AT`، `--dns-zonefile-effective-at` | فرض الطابع الزمني `effective_at` (RFC3339) بدلاً من أول ظهور للنافذة المقطوعة. |
| `DNS_ZONEFILE_PROOF`، `--dns-zonefile-proof` | تجاوز الإثبات الحرفي المسجل في البيانات الوصفية. |
| `DNS_ZONEFILE_CID`، `--dns-zonefile-cid` | تجاوز تسجيل CID في البيانات الوصفية. |
| `DNS_ZONEFILE_FREEZE_STATE`، `--dns-zonefile-freeze-state` | حالة التجميد (ناعم، صلب، ذوبان، مراقبة، طوارئ). |
| `DNS_ZONEFILE_FREEZE_TICKET`، `--dns-zonefile-freeze-ticket` | تذكرة مرجعية للوصي/المجلس لتجميدها. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`، `--dns-zonefile-freeze-expires-at` | الطابع الزمني RFC3339 لذوبان الجليد. |
| `DNS_ZONEFILE_FREEZE_NOTES`، `--dns-zonefile-freeze-note` | ملاحظات تجميد الإضافات (بيئة منفصلة عن بعضها البعض أو علامة قابلة للتكرار). |
| `DNS_GAR_DIGEST`، `--dns-gar-digest` | ملخص BLAKE3-256 (ست عشري) من علامة الحمولة GAR. يتطلب تقديم بوابة الارتباطات. |

أضاءت إجراءات GitHub لسير العمل هذه القيم من خلال أسرار الريبو لكل إنتاج دبوس
يتم إصدار ملف منطقة القطع الأثرية تلقائيًا. قم بتكوين الأسرار التالية (القيم الممكنة
تحتوي على قوائم منفصلة بشكل متكرر للأبطال ذات القيم المتعددة):

| سر | لكن |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`، `DOCS_SORAFS_DNS_ZONE` | يمر إنتاج اسم المضيف/المنطقة بالمساعد. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | الاسم المستعار عند الطلب المخزون في الواصف. |
| `DOCS_SORAFS_ZONEFILE_IPV4`، `DOCS_SORAFS_ZONEFILE_IPV6` | يسجل IPv4/IPv6 كناشر. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | خيار CNAME المستهدف. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | دبابيس SPKI قاعدة64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | مقبلات TXT الإضافات. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | تجميد البيانات الوصفية المسجلة في القرص. |
| `DOCS_SORAFS_GAR_DIGEST` | ملخص BLAKE3 في علامة الحمولة GAR السداسية. |

في حالة إلغاء الإدخال `.github/workflows/docs-portal-sorafs-pin.yml`، قم بتوفير المدخلات
`dns_change_ticket` و`dns_cutover_window` ليجد الواصف/ملف المنطقة
نافذة جيدة. اترك مقاطع الفيديو فريدة من نوعها للتشغيل الجاف.

نوع الاستدعاء (en ligne avec le runbookowner SN-7):

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
  ...autres flags...
```

يقوم المساعد تلقائيًا باسترداد تذكرة التغيير مثل إدخال TXT وتسجيل أول ظهور له
يتم قطع النافذة بالطابع الزمني `effective_at`. أكمل سير العمل، ثم انظر
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### ملاحظة حول تفويض DNS العام

لا يحدد الهيكل العظمي لملف المنطقة ما هي التسجيلات التلقائية له
المنطقة. يجب عليك إعادة تكوين تفويض NS/DS لمنطقة الوالدين في المنزل
مسجلك أو مزود DNS الخاص بك حتى يتمكن الإنترنت العام من العثور على الخوادم
دي الأسماء.

- لإضافة المقاطع إلى قمة/TLD، استخدم ALIAS/ANAME (selon le fournisseur) أو
  نشر تسجيلات A/AAAA عبر بوابة IP Anycast.
- من أجل المجالات الفرعية، قم بنشر CNAME مقابل اشتقاق المضيف الجميل
  (`<fqdn>.gw.sora.name`).
- L'hote canonique (`<hash>.gw.sora.id`) يقع في الجزء السفلي من نطاق البوابة وآخرون
  n'est pas publie dans votre Zone publique.

### قالب بوابة الرؤوس

مساعد النشر يصدر أيضًا `portal.gateway.headers.txt` وآخرون
`portal.gateway.binding.json`، اثنان من المصنوعات اليدوية التي تلبي متطلبات DG-3
صب ربط محتوى البوابة:

- `portal.gateway.headers.txt` يحتوي على كتلة كاملة من رؤوس HTTP (متضمنة)
  `Sora-Name`، `Sora-Content-CID`، `Sora-Proof`، CSP، HSTS، والواصف
  `Sora-Route-Binding`) ستؤدي حافة البوابات إلى حدوث بعض الاستجابة.
- `portal.gateway.binding.json` قم بتسجيل معلومات meme على شكل آلة مرئية
  حتى تتمكن من مقارنة تذاكر التغيير والأتمتة
  روابط المضيف/cid sanscraper lasortie shell.

إنها أنواع يتم تشغيلها تلقائيًا عبر
`cargo xtask soradns-binding-template`
والتقط الاسم المستعار، وملخص البيان، وبوابة اسم المضيف المقدمة أ
`sorafs-pin-release.sh`. لتجديد كتلة الرؤوس أو تخصيصها، قم بفكها:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```قم بتمرير `--csp-template`، `--permissions-template`، أو `--hsts-template` للتجاوز
قوالب الرؤوس الافتراضية عند النشر بناءً على التوجيهات
المكملات الغذائية؛ الجمع بين المفاتيح `--no-*` الموجودة للحذف
استكمال رأس الأمم المتحدة.

قم بتوصيل مقتطف الرؤوس إلى طلب تغيير CDN وقم بتنشيط مستند JSON
بوابة أتمتة خط الأنابيب من أجل أن يتوافق الترويج الساخن مع
دليل الإفراج.

يقوم البرنامج النصي للإصدار بتنفيذ مساعد التحقق تلقائيًا من أجلهم
تتضمن التذاكر DG-3 أحدث الأدلة. أعد تنفيذ الأمر يدويًا
يمكنك تعديل ربط JSON بشكل رئيسي:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

الأمر بفك تشفير الحمولة `Sora-Proof` يضغط، والتحقق من بيانات التعريف
`Sora-Route-Binding` يتوافق مع CID للبيان + اسم المضيف، وصدى الصوت
إذا تم اشتقاق رأس واحد. أرشفة وحدة التحكم مع العناصر الأخرى
النشر عند إطلاق الأمر عبر CI للمراجعين DG-3
هذا يعني أن الربط صالح قبل القطع.

> **تكامل واصف DNS:** `portal.dns-cutover.json` أثناء الصيانة
> قسم واحد `gateway_binding` يشير إلى هذه المصنوعات اليدوية (الطرق، المحتوى CID،
> حالة الإثبات ونموذج الرؤوس الحرفية) **و** مقطع `route_plan`
> الذي يشير إلى `gateway.route_plan.json` بالإضافة إلى قوالب الرؤوس
> الأصل والتراجع. تشمل هذه الكتل في كل تذكرة DG-3 من أجلهم
> المراجعون قادرون على مقارنة القيم الدقيقة `Sora-Name/Sora-Proof/CSP` et
> تأكيد أن خطط الترويج/التراجع تتوافق مع حزمة الأدلة
> بدون فتح الأرشيف.

##القطعة 9 - تنفيذ شاشات النشر

تتطلب خريطة طريق العنصر **DOCS-3c** دليلاً لمواصلة عملية النقل والوكيل جربه وما إلى ذلك
تبقى بوابة الروابط ثابتة بعد الإصدار. قم بتشغيل وحدة المراقبة
ما عليك سوى متابعة الأشرطة 7-8 وفرع برامجك:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` قم بشحن ملف التكوين (voir
  `docs/portal/docs/devportal/publishing-monitoring.md` للمخطط) وآخرون
  تنفيذ عمليات التحقق الثلاثة: تحقيقات المسارات المحمولة + التحقق من صحة CSP/Permissions-Policy،
  تحقيقات الوكيل جربه (الخيار عبر نقطة النهاية `/metrics`)، والتحقق
  بوابة الربط (`cargo xtask soradns-verify-binding`) التي تتطلب الصيانة
  حضور وقيمة Sora-Content-CID مع الشيكات المستعارة/البيان.
- الأمر بإرجاع غير الصفر عند صدى المسبار لـ CI، cron jobs، ou
  يمكن لمشغلي دفتر التشغيل إيقاف الإصدار قبل الترويج للأسماء المستعارة.
- يقوم Passer `--json-out` بكتابة استئناف JSON فريد مع الوضع المتساوي؛ `--evidence-dir`
  إميت `summary.json`، `portal.json`، `tryit.json`، `binding.json`، و`checksums.sha256`
  لكي يتمكن المراجعون من مقارنة النتائج دون إعادة النظر في الشاشات.
  أرشفة هذا المرجع سو `artifacts/sorafs/<tag>/monitoring/` مع الحزمة Sigstore
  واصف DNS.
- قم بتضمين شاشة العرض، تصدير Grafana (`dashboards/grafana/docs_portal.json`)،
  ومعرف الحفر Alertmanager في إصدار التذكرة حتى يكون SLO DOCS-3c كذلك
  قابلة للتدقيق بالإضافة إلى التأخير. كتاب قواعد مراقبة النشر يحتوي على فيتامين أ
  `docs/portal/docs/devportal/publishing-monitoring.md`.

تتطلب تحقيقات البوابة HTTPS وحذف عناوين URL الأساسية `http://` sauf si
تم تحديد `allowInsecureHttp` في شاشة التكوين؛ إنتاج / تنظيم Gardez les Cibles
sur TLS وn'activez l'override que pour des Previews locales.

أتمتة الشاشة عبر `npm run monitor:publishing` وBuildkite/cron مرة واحدة
البوابة الإلكترونية. أمر meme، يشير إلى إنتاج عناوين URL، بالإضافة إلى عمليات التحقق
إصدارات sante continus entre.

## الأتمتة مع `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` يغلف الأشرطة 2-6. إل:

1. أرشفة `build/` في محدد كرة القطران،
2. تنفيذ `car pack`، `manifest build`، `manifest sign`، `manifest verify-signature`،
   وآخرون `proof verify`,
3. تنفيذ الخيار `manifest submit` (بما في ذلك الاسم المستعار للربط) أثناء
   أوراق الاعتماد Torii تقدم، إلخ
4. أكتب `artifacts/sorafs/portal.pin.report.json`، الخيار
  `portal.pin.proposal.json`، واصف DNS (الإرسال المسبق)،
  وآخرون بوابة الربط (`portal.gateway.binding.json` بالإضافة إلى كتلة الرؤوس)
  من أجل أن تتمكن معدات الحوكمة والشبكات والعمليات من مقارنة الأدلة
  بدون fouiller les logs CI.

تعريف `PIN_ALIAS`، `PIN_ALIAS_NAMESPACE`، `PIN_ALIAS_NAME`، وآخرون (عنصر اختياري)
`PIN_ALIAS_PROOF_PATH` قبل استدعاء البرنامج النصي. استخدم `--skip-submit` للتجفيف
يدير؛ يتم إجراء سير العمل على GitHub عبر الإدخال `perform_submit`.

## الشريط 8 - نشر المواصفات OpenAPI والحزم SBOM

يتطلب DOCS-7 إنشاء البوابة والمواصفات OpenAPI والعناصر الأثرية SBOM العابرة
تحديد خط أنابيب ميمي. المساعدين الموجودين يغطيون الثلاثة:

1. **تجديد المواصفات وتوقيعها.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   قم بإصدار إصدار التسمية عبر `--version=<label>` عندما ترغب في الاحتفاظ بلقطة واحدة
   تاريخي (على سبيل المثال `2025-q3`). يقوم المساعد بكتابة اللقطة داخل التطبيق
   `static/openapi/versions/<label>/torii.json`، أنظر إليه
   `versions/current`، وقم بتسجيل البيانات التعريفية (SHA-256، حالة البيان، الطابع الزمني
   لقد مر يوم) في `static/openapi/versions.json`. يطوّر Portail مؤشر CET المضاء
   لكي تعرض اللوحات Swagger/RapiDoc منتقي الإصدار وتعرضه
   ملخص/la التوقيع المساعد مضمنة. يحافظ Omettre `--version` على ملصقات الإصدار
   سابقة ولا تكرر أن المؤشرات `current` + `latest`.

   يلتقط البيان خلاصات SHA-256/BLAKE3 حتى تتمكن البوابة من قبولها
   الرؤوس `Sora-Proof` لـ `/reference/torii-swagger`.

2. **Emettre les SBOMs CycloneDX.** يتم إطلاق خط الأنابيب في وقت لاحق من SBOMs syft
   سيلون `docs/source/sorafs_release_pipeline_plan.md`. جارديز لا طلعة جوية
   ما قبل القطع الأثرية للبناء:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Empaqueter chaque payload en CAR.**

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

   تابع الميمات والمقاطع `manifest build` / `manifest sign` كما هو الحال مع مدير الموقع،
   وضبط الأسماء المستعارة للمنتج (على سبيل المثال، `docs-openapi.sora` للمواصفات
   `docs-sbom.sora` من أجل حزمة SBOM Signe). Garder des aliases المتميزة بشكل مستمر
   SoraDNS وGARs وتراجع التذاكر يحد من الحمولة تمامًا.

4. **إرسال وربط.** أعد استخدام الملف التلقائي الموجود والحزمة Sigstore، ثم قم بالتسجيل
   يتم إصدار مجموعة الأسماء المستعارة في قائمة التحقق حتى يتمكن المدققون من متابعة ذلك
   nom Sora Mappe vers quel Digest Mante.

أرشفة بيانات المواصفات/SBOM مع بوابة البناء تضمن إصدار كل تذكرة
تحتوي على مجموعة كاملة من القطع الأثرية بدون إعادة تنفيذ الباكر.

### مساعد الأتمتة (البرنامج النصي CI/الحزمة)

`./ci/package_docs_portal_sorafs.sh` يقوم بتدوين الأشرطة 1-8 لخريطة طريق العنصر
**DOCS-7** قابل للتنفيذ بأمر واحد. لو المساعد:

- تنفيذ متطلبات التحضير للبوابة (`npm ci`، مزامنة OpenAPI/norito، عناصر واجهة المستخدم للاختبارات)؛
- قم بإصدار CARs وأزواج البيانات في الباب، OpenAPI وSBOM عبر `sorafs_cli`؛
- تنفيذ الخيار `sorafs_cli proof verify` (`--proof`) والتوقيع Sigstore
  (`--sign`، `--sigstore-provider`، `--sigstore-audience`)؛
- إيداع جميع القطع الأثرية `artifacts/devportal/sorafs/<timestamp>/` et
  اكتب `package_summary.json` حتى يتمكن CI/outillage من إدخال الحزمة؛ وآخرون
- rafraichit `artifacts/devportal/sorafs/latest` للمؤشر مقابل التنفيذ الأخير.

مثال (اكتمل خط الأنابيب مع Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

استخدامات الأعلام:

- `--out <dir>` - تجاوز جذر العناصر (الحفاظ الافتراضي على الطابع الزمني للملفات).
- `--skip-build` - إعادة استخدام `docs/portal/build` الموجود (مفيد عندما لا يمكن استخدام CI
  إعادة بناء سبب المرايا في وضع عدم الاتصال).
- `--skip-sync-openapi` - تجاهل `npm run sync-openapi` و`cargo xtask openapi`
  لا يمكن الانضمام إلى صناديق.io.
- `--skip-sbom` - تجنب طلب `syft` عند عدم تثبيت الملف الثنائي (سجل البرنامج النصي)
  إعلان عن مكان).
- `--proof` - تنفيذ `sorafs_cli proof verify` لكل زوج من السيارات/البيان. الحمولات
  تظهر الملفات المتعددة بشكل ملح لدعم خطة القطع في CLI، دون ترك علامة CLI
  قم بإلغاء التنشيط في حالة اكتشاف الأخطاء `plan chunk count` والتحقق يدويًا
  une fois le gate upstream livre.
- `--sign` - الاستدعاء `sorafs_cli manifest sign`. أرسل رمزًا مميزًا عبر
  `SIGSTORE_ID_TOKEN` (ou `--sigstore-token-env`) أو اترك CLI للاسترداد عبر
  `--sigstore-provider/--sigstore-audience`.

لإنتاج المصنوعات اليدوية، استخدم `docs/portal/scripts/sorafs-pin-release.sh`.
يتم تغليف الباب بصيانة، OpenAPI وSBOM، قم بتوقيع كل بيان، وقم بالتسجيل
أصول بيانات التعريف التكميلية في `portal.additional_assets.json`. لو المساعد
فهم المقابض الاختيارية التي تحتوي على أداة حزم CI بالإضافة إلى المفاتيح الجديدة
`--openapi-*` و`--portal-sbom-*` و`--openapi-sbom-*` لتعيين مجموعات الأسماء المستعارة
حسب الصنعة، تجاوز مصدر SBOM عبر `--openapi-sbom-source`، تأكد من التأكد
الحمولات (`--skip-openapi`/`--skip-sbom`)، والمؤشر مقابل ثنائي `syft` غير افتراضي
مع `--syft-bin`.

يعرض البرنامج النصي كل أمر يتم تنفيذه؛ انسخ السجل في إصدار التذكرة
مع `package_summary.json` حتى يتمكن المراجعون من مقارنة ملخصات CAR،
البيانات التعريفية للخطة، وتجزئات الحزمة Sigstore دون مشاركة قذيفة مخصصة.## الشريط 9 - بوابة التحقق + SoraDNS

قبل الإعلان عن عملية قطع، تأكد من حل الاسم المستعار الجديد عبر SoraDNS والبوابات
agraffent des profes frais:

1. ** منفذ مسبار البوابة. ** تمرين `ci/check_sorafs_gateway_probe.sh`
   `cargo xtask sorafs-gateway-probe` مع عرض توضيحي للتركيبات
   `fixtures/sorafs_gateway/probe_demo/`. من أجل نشر البكرات، قم بتوجيه المسبار
   مقابل اسم المضيف cible:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   يقوم المسبار بفك تشفير `Sora-Name` و`Sora-Proof` و`Sora-Proof-Status`.
   `docs/source/sorafs_alias_policy.md` وصدى عندما يظهر البيان،
   يتم اشتقاق TTLs أو الارتباطات من GAR.

   لإجراء فحوصات مفاجئة خفيفة الوزن (على سبيل المثال، عند ربط الحزمة فقط
   تم التغيير)، قم بتشغيل `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   يقوم المساعد بالتحقق من صحة حزمة الربط التي تم التقاطها وهو سهل الإصدار
   التذاكر التي تحتاج فقط إلى تأكيد ملزم بدلاً من تدريبات التحقيق الكاملة.

2. **التقاط دليل المثقاب.** لتشغيل المثاقب أو عمليات التشغيل الجافة PagerDuty،
   قم بتغليف المسبار مع `scripts/telemetry/run_sorafs_gateway_probe.sh --السيناريو
   طرح devportal -- ...`. رؤوس/سجلات المجمع مخزنة
   `artifacts/sorafs_gateway_probe/<stamp>/`، في يوم `ops/drill-log.md`، وآخرون
   (اختياري) قم بإلغاء قفل الخطافات أو حمولات PagerDuty. تعريف
   `--host docs.sora` للتحقق من نظام SoraDNS بدلاً من كود IP الثابت.

3. **التحقق من روابط DNS.** عندما تنشر الإدارة الاسم المستعار للدليل، قم بالتسجيل
   مرجع GAR للملف المرجعي للمسبار (`--gar`) وانضم إلى إصدار الأدلة.
   يمكن لمحلل المالكين تجديد إدخال meme عبر `tools/soradns-resolver` من أجل
   أؤكد أن المقبلات الموجودة في ذاكرة التخزين المؤقت تشرف على الظهور الجديد. قبل الانضمام إلى JSON،
   com.executez
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   من أجل التحقق من صحة تعيين مضيف التعيين في وضع عدم الاتصال، وبيان البيانات الوصفية والتسميات
   القياس عن بعد. يمكن للمساعد إنشاء سيرة ذاتية `--json-out` مع علامة GAR لتتمكن من ذلك
   المراجعون لديهم أدلة يمكن التحقق منها بدون فتح ملف ثنائي.
  عندما تقوم بإعادة إنشاء GAR الجديد، تفضل
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (الرجوع إلى `--manifest-cid <cid>` فريد عندما لا يكون الملف واضحًا
  متاح). المساعد يشتق بشكل رئيسي CID **و** يلخص توجيه BLAKE3
  depuis lemani JSON، double les spaces، deduplique les flags `--telemetry-label`،
  جرب التصنيفات واستخرج قوالب CSP/HSTS/Permissions-Policy افتراضيًا مسبقًا
  قم بكتابة JSON لكي تحدد الحمولة النافعة ما إذا كان المشغلون
  يلتقط التسميات من الأصداف المختلفة.

4. **الاسم المستعار لمراقب المقاييس.** Gardez `torii_sorafs_alias_cache_refresh_duration_ms`
   و`torii_sorafs_gateway_refusals_total{profile="docs"}` على الشاشة المعلقة على المسبار؛
   سلسلة Ces هي graphees dans `dashboards/grafana/docs_portal.json`.

## الشريط 10 – المراقبة وحزمة الأدلة

- **لوحات المعلومات.** التصدير `dashboards/grafana/docs_portal.json` (بوابة SLOs)،
  `dashboards/grafana/sorafs_gateway_observability.json` (بوابة زمن الوصول +
  دليل صحي)، و`dashboards/grafana/sorafs_fetch_observability.json`
  (سانتي أوركسترا) صب إطلاق سراح شاك. Joignez les Exports JSON au Ticket Release
  لكي يتمكن المراجعون من تجديد الطلبات Prometheus.
- **مسبار المحفوظات.** حفظ `artifacts/sorafs_gateway_probe/<stamp>/` في git-annex
  أو دلو الأدلة الخاص بك. يتضمن استئناف التحقيق والرؤوس وحمولة PagerDuty
  التقاط القياس عن بعد على قدم المساواة مع النص.
- **إصدار الحزمة.** قم بتخزين استئناف CAR du portail/SBOM/OpenAPI، تظهر الحزم،
  التواقيع Sigstore، `portal.pin.report.json`، تسجل مسبار Try-It، وتبلغ عن التحقق من الارتباط
  الطابع الزمني لملف sous (على سبيل المثال، `artifacts/sorafs/devportal/20260212T1103Z/`).
- **سجل الحفر.** عندما تقوم بمسح الخط من جزء من الحفر، اتركه
  `scripts/telemetry/run_sorafs_gateway_probe.sh` إضافة المقبلات إلى `ops/drill-log.md`
  من أجل أن يرضي هذا الدليل متطلبات الفوضى SNNet-5.
- **امتيازات التذكرة.** قم بالرجوع إلى معرفات اللوحة Grafana أو تصدير PNG المرفقات في الملف
  تذكرة التغيير، مع طريق التحقيق، حتى يتمكن المراجعون من الوصول إليها
  استرداد SLOs بدون الوصول إلى Shell.

## الشريط 11 - جلب لوحة النتائج متعددة المصادر والأدلة

يتطلب الناشر SoraFS صيانة دليل جلب متعدد المصادر (DOCS-7/SF-6)
مع خيارات DNS/البوابة ci-dessus. أبريل لو دبوس دو مانفيستي:

1. **Lancer `sorafs_fetch` ضد البيان المباشر.** استخدم خطة/بيان المصنوعات اليدوية
   المنتجات في Etapes 2-3 مع بوابة بيانات الاعتماد لكل موفر.
   استمر في جميع الطلعات حتى يتمكن المدققون من استعادة أثر القرار
   منسق:

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

   - استعادة مراجع مقدمي الإعلانات حسب البيان (على سبيل المثال
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     وتمريرها عبر `--provider-advert name=path` حتى تقوم لوحة النتائج بتقييمها
     نوافذ القدرة المحددة. استخدم
     `--allow-implicit-provider-metadata` **فريد** عندما تستمتع بالتركيبات في CI;
     يجب أن يشير إنتاج التدريبات إلى الإعلانات المصاحبة للدبوس.
   - عند الإشارة إلى المناطق الإضافية، كرر الأمر باستخدامها
     مجموعات موفري المراسلات من أجل كل ذاكرة تخزين مؤقت/اسم مستعار هي قطعة أثرية لجلب مرتبطة.

2. **أرشفة الطلعات.** حفظ `scoreboard.json`,
   `providers.ndjson` و`fetch.json` و`chunk_receipts.ndjson` داخل الملف الأدلة
   الافراج عن دو. تلتقط هذه الملفات وزن الأقران، وميزانية إعادة المحاولة، وزمن انتقال EWMA
   والإيصالات جزء من حزمة الإدارة التي يجب الاحتفاظ بها لـ SF-7.

3. **القياس عن بعد طوال اليوم.** استيراد الطلعات التي يتم جلبها من لوحة القيادة
   **SoraFS جلب إمكانية الملاحظة** (`dashboards/grafana/sorafs_fetch_observability.json`)،
   مراقبة `torii_sorafs_fetch_duration_ms`/`_failures_total` وموفر نطاق اللوحات
   صب ليه الشذوذ. التقط اللقطات Grafana من خلال إصدار التذكرة مع لوحة النتائج.

4. **اختبار قواعد التنبيهات.** تنفيذ `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   للتحقق من حزمة التنبيهات Prometheus قبل إيقاف الإصدار. استمتع بالطلعة الجوية
   promtool au Ticket pour que les reviewers DOCS-7 يؤكد que les notifications de stand et
   الجيوش الباقية ذات الموفر البطيء.

5. **المتفرِّع في CI.** يبقي سير العمل على دبوس البوابة شريطًا `sorafs_fetch` في نهاية الأمر
   الإدخال `perform_fetch_probe`; activez-le pour les runstaging/production afin que
   يتم جلب الأدلة ويتم إنتاجها من خلال الحزمة الواضحة دون تدخل يدوي.
   يمكن أن تعمل التدريبات المحلية على إعادة استخدام النص البرمجي للميمات وتصدير بوابة الرموز المميزة وما إلى ذلك
   تم تحديد `PIN_FETCH_PROVIDERS` في قائمة موفري الخدمة المنفصلين بشكل افتراضي.

## الترويج والمراقبة والتراجع

1. **الترويج:** احصل على أسماء مستعارة مميزة للتمثيل والإنتاج. الترويج أون
   إعادة التنفيذ `manifest submit` مع ظهور/حزمة meme، والتغيير
   `--alias-namespace/--alias-name` للمؤشر مقابل إنتاج الاسم المستعار. سيلا تهرب
   de recontruire ou re-signer une fois que QA approuve le pin staging.
2. **المراقبة:** قم باستيراد سجل دبوس لوحة القيادة
   (`docs/source/grafana_sorafs_pin_registry.json`) بالإضافة إلى المسابير المحددة
   (التقرير `docs/portal/docs/devportal/observability.md`). تنبيه حول الانجراف الاختباري،
   تحقيقات أصداء أو بلدان جزر المحيط الهادئ من إعادة المحاولة إثبات.
3. **التراجع:** للرجوع للخلف، أو استعادة البيان السابق (أو التقاعد
   الاسم المستعار كورانت) مع `sorafs_cli manifest submit --alias ... --retire`.
   احصل دائمًا على الحزمة الأخيرة التي تعتبر جيدة واستئناف السيارة من أجلها
   يمكن استعادة البراهين إذا كانت سجلات CI تدور.

## نموذج سير العمل CI

الحد الأدنى، خط الأنابيب Votre doit:

1. Build + lint (`npm ci`، `npm run build`، إنشاء المجاميع الاختبارية).
2. قم بتجميع البيانات (`car pack`) وحساب البيانات.
3. المُوقع عبر الرمز المميز OIDC du job (`manifest sign`).
4. أداة تحميل العناصر (السيارة، البيان، الحزمة، الخطة، السير الذاتية) للتدقيق.
5. سجل Soumettre au pin:
   - سحب الطلبات -> `docs-preview.sora`.
   - العلامات / فروع المحميين -> إنتاج الاسم المستعار الترويج.
6. قم بتنفيذ المسابير + بوابات إثبات التحقق قبل النهاية.

`.github/workflows/docs-portal-sorafs-pin.yml` قم بتوصيل جميع هذه الأشرطة للإصدارات اليدوية.
سير العمل:

- إنشاء/اختبار البوابة،
- البناء عبر `scripts/sorafs-pin-release.sh`،
- قم بالتوقيع/التحقق من بيان الحزمة عبر GitHub OIDC،
- تحميل CAR/manifeste/bundle/plan/resumes comme artifacts، et
- (اختياري) بعض البيانات + الاسم المستعار المرتبط عندما يتم تقديم الأسرار.

تكوين الأسرار/المتغيرات اللاحقة قبل إلغاء الوظيفة:

| الاسم | لكن |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | المضيف Torii الذي كشف `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | معرف العصر المسجل مع عمليات الإرسال. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autorite de التوقيع لبيان التسليم. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | الاسم المستعار Tuple يكمن في البيان عندما يكون `perform_submit` هو `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | تشفير الاسم المستعار المضاد للحزمة en base64 (optionnel؛ omettre pour sauter alias ملزمة). |
| `DOCS_ANALYTICS_*` | يتم إعادة استخدام تحليلات نقاط النهاية/موجودات الاختبار على قدم المساواة مع سير العمل الآخر. |

قم بإلغاء قفل سير العمل عبر إجراءات واجهة المستخدم:

1. المورد `alias_label` (على سبيل المثال: `docs.sora.link`)، خيار `proposal_alias`،
   وتجاوز خيار `release_tag`.
2. اترك `perform_submit` غير محدد لإنشاء عناصر بدون لمس Torii
   (مفيد للتشغيل الجاف) أو نشط للنشر المباشر على تكوين الاسم المستعار.`docs/source/sorafs_ci_templates.md` يقوم بتوثيق المستندات المساعدة العامة لـ CI
المشاريع خارج نطاق الريبو، لكن بوابة سير العمل تفضل ذلك
من أجل الإصدارات اليومية.

## قائمة المراجعة

- [ ] `npm run build`، `npm run test:*`، و`npm run check:links` هي verts.
- [ ] يلتقط `build/checksums.sha256` و`build/release.json` داخل العناصر.
- [ ] CAR والخطة والبيان واستئناف الأنواع `artifacts/`.
- [ ] الحزمة Sigstore + مخزون التوقيع المنفصل مع السجلات.
- [ ] `portal.manifest.submit.summary.json` و`portal.manifest.submit.response.json`
      يلتقط عند التقديمات بدلا من ذلك.
- [ ] `portal.pin.report.json` (والخيار `portal.pin.proposal.json`)
      أرشيف مع القطع الأثرية CAR/البيان.
- [ ] سجلات أرشيفات `proof verify` و`manifest verify-signature`.
- [ ] لوحات المعلومات Grafana فقدت كل يوم + تحقيقات Try-It reussis.
- [ ] التراجع عن الملاحظات (معرف البيان السابق + الاسم المستعار الملخص) مفاصل au
      الافراج عن التذكرة.