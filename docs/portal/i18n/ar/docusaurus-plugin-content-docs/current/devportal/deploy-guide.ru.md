---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## ملاحظة

يقوم هذا القرص بتحويل النقاط الصغيرة إلى البطاقات التالية **DOCS-7** (النشر SoraFS) و **DOCS-8** (دبوس الأتمتة في CI/CD) بشكل عملي إجراءات بوابة المصممين. من خلال تحسين البنية/اللينت، التعبئة SoraFS، نشر البيان من خلال Sigstore، إنتاج الاسم المستعار، التحقق، تم تدريب المدربين على أن تكون كل معاينة وإصدار عبارة عن مؤثرات صوتية وسمعية.

يقترح ذلك أنك الآن ثنائي `sorafs_cli` (مرتبط بـ `--features cli`)، يمكنك الوصول إلى نقطة النهاية Torii باستخدام سجل الدبوس الصحيح و OIDC بيانات رائعة لـ Sigstore. الأسرار الطويلة (`IROHA_PRIVATE_KEY`، `SIGSTORE_ID_TOKEN`، الرموز المميزة Torii) موجودة في قبو CI؛ يمكن للمنافذ المحلية أن تدعم تصديرها في Shell.

## خدمة مسبقة

- العقدة 18.18+ مع `npm` أو `pnpm`.
- `sorafs_cli` من `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- عنوان URL Torii، الذي يتم الكشف عنه `/v1/sorafs/*`، بالإضافة إلى مفتاح القفل/المفتاح الخاص، الذي يمكن من خلاله إرسال البيانات و الاسم المستعار.
- مُصدر OIDC (GitHub Actions وGitLab وهوية عبء العمل وما إلى ذلك) للإصدار `SIGSTORE_ID_TOKEN`.
- اختياريًا: `examples/sorafs_cli_quickstart.sh` للتشغيل الجاف و`docs/source/sorafs_ci_templates.md` لتخطيط سير عمل GitHub/GitLab.
- قم بإنشاء تغييرات OAuth جربها (`DOCS_OAUTH_*`) وقم بتفعيلها
  [قائمة التحقق من تعزيز الأمان](./security-hardening.md) قبل إنتاج مختبر التقدم. يتم تشغيل البوابة الإلكترونية مرة أخرى، في حالة الخروج المؤقت أو اختيار معلمات TTL/السحب للإغلاق؛ يستخدم `DOCS_OAUTH_ALLOW_INSECURE=1` فقط للمعاينة المحلية الفريدة. قم بتجسيد اختبار القلم في نتيجة المهمة.

## الجزء 0 - تثبيت بروكسي الحزمة جربه

قبل تقديم المعاينة في Netlify أو البوابة، قم بإلغاء تثبيت المستندات الخاصة بـ Try it proxy واستوعب بيان OpenAPI في تحديد الحزمة:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` يقوم بنسخ مساعدي الوكيل/المسبار/الاستعادة، وتحقق من كتابة OpenAPI وأرسل `release.json` بالإضافة إلى `checksums.sha256`. قم بنسخ هذه الحزمة إلى تذكرة بوابة Netlify/SoraFS، بحيث يمكن للمسجلين الاستفادة من بروكسيات ورسائل البريد الإلكتروني Torii الهدف بدون حواجز. تم تأمين الحزمة أيضًا، بما في ذلك العملاء المميزين لحاملها (`allow_client_auth`)، لخطة الطرح وتوفير المزامنة المناسبة لـ CSP.

## الجزء 1 - بناء البوابة والوبر

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

`npm run build` غلق تلقائي `scripts/write-checksums.mjs`، تم إنشاؤه:

- `build/checksums.sha256` - بيان SHA256 لـ `sha256sum -c`.
- `build/release.json` - ميتاداني (`tag`، `generated_at`، `source`) لكل سيارة/مانيفست.

يمكنك أرشفة هذا الملف في ملخص CAR حتى يتمكن المراجعون من معاينة العناصر بدون حواجز.

## الجزء 2 - تعبئة الأصول الإحصائية

قم بتثبيت CAR packer لكتالوج المشروبات الغازية Docusaurus. أرسل المثال التوضيحي جميع المصنوعات اليدوية إلى `artifacts/devportal/`.

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

ملخص JSON يصلح شيسلو شانكوف، والتفاصيل والملصقات لإثبات التخطيط، والتي تستخدم بعد ذلك لوحات معلومات `manifest build` وCI.

## الجزء 2ب - قم بتثبيت OpenAPI ورفاق SBOM

يجب أن تقوم DOCS-7 بنشر بوابة الموقع واللقطة OpenAPI وحمولات SBOM كبيانات إضافية يمكن إضافة البوابات إليها رؤوس `Sora-Proof`/`Sora-Content-CID` لكل قطعة أثرية. مساعد التحرير (`scripts/sorafs-pin-release.sh`) يقوم بحزم كتالوج OpenAPI (`static/openapi/`) وSBOM من `syft` في السيارة الإضافية `openapi.*`/`*-sbom.*` وأكتب الوصفة في `artifacts/sorafs/portal.additional_assets.json`. في أقرب وقت ممكن، قم بإلقاء الضوء على 2-4 لكل الحمولة النافعة مع البادئات والبديلات (على سبيل المثال `--car-out "$OUT"/openapi.car` plus) `--metadata alias_label=docs.sora.link/openapi`). قم بتسجيل كل بيان/اسم مستعار في Torii (الموقع، OpenAPI، بوابة SBOM، OpenAPI SBOM) إلى DNS المنع، لإمكانية البوابة قم بإدراج البروفات المدبسة لجميع المصنوعات اليدوية العامة.

## الجزء 3 - اقرأ البيان

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

قم بتثبيت أعلام سياسة الدبوس أسفل الإصدار (على سبيل المثال، `--pin-storage-class hot` للطيور). متغير JSON اختياري، ولكنه سهل المراجعة.

## الجزء 4 - قم بالنشر من خلال Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

تحتوي الحزمة على بيان بيانات مرفق، وأطراف أصابع، ورمز BLAKE3 الخاص بـ OIDC، وغير مشمول في JWT. حزمة فارغة وتوقيع منفصل؛ يمكن أن تستخدم منتجات البيع هذه المنتجات في أي مكان آخر. يمكن للمزودين المحليين تغيير علامة الموفر إلى `--identity-token-env` (أو إضافة `SIGSTORE_ID_TOKEN` في الملحقات)، عندما يكون مساعد OIDC الداخلي رمز مميز.

## الجزء 5 - قم بالتسجيل في رقم التعريف الشخصي

قم بمعالجة البيان التفصيلي (وخطة القطعة) في Torii. ابدأ الآن في كتابة الملخص لتتمكن من كتابة التسجيل/الاسم المستعار.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority soraカタカナ... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

عند المعاينة التمهيدية أو الاسم المستعار canary (`docs-preview.sora`)، يمكنك الانتقال باستخدام اسم مستعار فريد، بحيث يمكن لـ QA التحقق من المحتوى قبل ترويج الإنتاج.

الاسم المستعار الملزم trебует трае полей: `--alias-namespace`، `--alias-name` و`--alias-proof`. تحدد الحوكمة حزمة إثبات (base64 أو Norito بايت) بعد نشر الاسم المستعار؛ اكشف عن أسرار CI وتحدث عن الملف الموجود على `manifest submit`. قم بتثبيت الاسم المستعار للأعلام إذا كنت ترغب في كسر البيان بدون تغيير DNS.

## الجزء 5 ب - اقتراح مقترح للحوكمة

يجب أن يدعم كل بيان اقتراحًا برلمانيًا من أجل أن يتمكن أي شخص من الحصول على سورا من التوسيع دون التنازل عنه كلمة مميزة. بعد الإرسال/التوقيع، اختر:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` يكمل التعليمات القانونية `RegisterPinManifest`، ملخص العناصر، تلميح السياسة والاسم المستعار. قم باستخراج تذكرة الحكم أو بوابة البرلمان حتى يتمكن المندوبون من مراقبة الحمولة دون الحواجز. لا تستخدم الأوامر المفاتيح Torii، حيث يمكن لأي شخص آخر الموافقة على الاقتراح محليًا.

## الجزء 6 - التحقق من البراهين وأجهزة القياس عن بعد

بعد التحقق من تحديد الدبوس:

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

- الشرائح لـ `torii_sorafs_gateway_refusals_total` و`torii_sorafs_replication_sla_total{outcome="missed"}`.
- اضغط على `npm run probe:portal` للتحقق من وكيل Try-It وتسجيل الدخول إلى جميع المحتويات.
- مراقبة الأدلة الرصينة من
  [النشر والمراقبة](./publishing-monitoring.md)، من أجل بوابة إمكانية المراقبة DOCS-3c. يقوم المساعد ببدء تشغيل عدد قليل من `bindings` (الموقع، OpenAPI، Portal SBOM، OpenAPI SBOM) وتريد `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` للمضيف الكامل عبر حارس `hostname`. أرسل أدناه ملخص JSON وحزمة الأدلة (`portal.json`، `tryit.json`، `binding.json`، `checksums.sha256`) في نسخة الكتالوج:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## الجزء 6 أ - بوابة تخطيط الشهادات

قم بتنسيق خطة TLS SAN/التحدي لإنشاء حزم GAR، بحيث تعمل بوابة الأوامر ومعتمدو DNS باستخدام دليل واحد أو آخر. يقوم المساعد الجديد بفحص مدخلات DG-3، وإعادة تمثيل مضيفي أحرف البدل الأساسية، وشبكات SAN المضيفة الجميلة، وتسميات DNS-01، وتحديات ACME الموصى بها:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

التزم بـ JSON مع حزمة الإصدار (أو قم بتركيب تذكرة تغيير)، حتى يتمكن المشغلون من تثبيت اتصال SAN في التكوين `torii.sorafs_gateway.acme`، ومراجعي GAR يمكنك الرجوع إلى التعيينات الأساسية/الجميلة بدون إغلاق لاحق. قم بإضافة `--name` الإضافية لكل ملحقة في إصدار واحد.

## الجزء 6 ب - التعرف على تعيينات المضيف الأساسية

قبل أن تقوم بإلغاء تحديد حمولات GAR، قم بتأكيد تحديد تعيين المضيف لكل اسم مستعار. `cargo xtask soradns-hosts` قم باختيار الكلمة `--name` في التسمية الأساسية (`<base32>.gw.sora.id`)، وحذف أحرف البدل المطلوبة (`*.gw.sora.id`) ثم قم باختيار مضيف جميل (`<alias>.gw.sora.name`). ساهم في إنشاء القطع الأثرية في الإصدار بحيث يمكن لمراجعي DG-3 استكشاف رسم الخرائط بشكل مناسب مع تقديم GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

استخدم `--verify-host-patterns <file>` لتتمكن من الاتصال بسهولة عندما لا يتم توصيل GAR أو بوابة ربط JSON بالمضيفين الملتزمين. يقوم المساعد بتجريب عدد قليل من الملفات والتحقق السهل وقالب GAR و`portal.gateway.binding.json` في أمر واحد:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

قم بتدوين ملخص JSON وسجل التحقق في تذكرة تغيير DNS/البوابة حتى يتمكن المدققون من التحقق من المضيفين الأساسيين/أحرف البدل/الجميلة دون الحاجة إلى أي اتصال لاحق. قم بتمرير الأمر عند إضافة الاسم المستعار الجديد إلى تحديثات GAR التي تتبعها كأدلة.

## الجزء 7 - تصميم واصف قطع DNS

تحتاج عمليات قطع الإنتاج إلى حزمة تغيير الصوت. بعد التقديم الناجح (الاسم المستعار الملزم)، تم إنشاء المساعد `artifacts/sorafs/portal.dns-cutover.json`، التثبيت:- ربط الاسم المستعار للبيانات الوصفية (مساحة الاسم/الاسم/الدليل، ملخص البيان، Torii URL، العصر المقدم، السلطة)؛
- سياق الإصدار (العلامة، تسمية الاسم المستعار، البيان/خطة السيارة، خطة القطعة، حزمة Sigstore)؛
- مؤشرات التحقق (أمر التحقيق، الاسم المستعار + نقطة النهاية Torii)؛ و
- شريط التحكم في التغيير الاختياري (معرف التذكرة، نافذة التحويل، جهة اتصال العمليات، اسم/منطقة مضيف الإنتاج)؛
- ترويج مسار البيانات الوصفية من رأس `Sora-Route-Binding` (المضيف/CID المتعارف عليه، رأس الإدخال + الربط، أوامر التحقق)، لترويج GAR والتدريبات الاحتياطية التي يتم إجراؤها على دليل واحد؛
- العناصر الاصطناعية لخطة المسار (`gateway.route_plan.json`، قوالب الرأس ورؤوس التراجع الاختيارية)، لتغيير التذاكر وخطافات CI التي يمكنها التحقق من إمكانية استخدام حزمة DG-3 على خطط الترويج/التراجع الأساسية؛
- البيانات التعريفية الاختيارية لإبطال ذاكرة التخزين المؤقت (تطهير نقطة النهاية، متغير المصادقة، حمولة JSON والمثال `curl`)؛
- تلميحات التراجع في الواصف السابق (علامة الإصدار وملخص البيان)، لتغيير التذاكر، لإصلاح تحديد الإجراء الاحتياطي.

إذا قمت بتحرير تطهير ذاكرة التخزين المؤقت، قم بتعديل الخطة الأساسية باستخدام الواصف:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

قم بقص `portal.cache_plan.json` إلى حزمة DG-3 لكي يتمكن المشغلون من تحديد المضيفين/المسارات (وتلميحات المصادقة المرغوبة) في `PURGE` خطأ. يمكن تخصيص واصف قسم ذاكرة التخزين المؤقت الاختياري لهذا الملف على النحو الأمثل، ليرى مراجعو التحكم في التغيير أن نقاط النهاية يتم حذفها عند الاستبدال.

تتضمن حزمة DG-3 بالإضافة إلى الترويج المطلوب + قائمة التحقق من التراجع. قم بصياغة هذا من خلال `cargo xtask soradns-route-plan` بحيث يمكن لمراجعي التحكم في التغيير أن يتفوقوا على بعض الاختبارات المبدئية/التحويل/التراجع عن الاسم المستعار:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` يقوم بإصلاح المضيفين الأساسيين/الجميلين، وتذكيرات التحقق من الصحة المرحلية، وتحديثات ربط GAR، وعمليات تطهير ذاكرة التخزين المؤقت، وإجراءات التراجع. قم بتجسيد نفسك باستخدام عناصر GAR/binding/cutover للمساعدة في تغيير التذكرة، بحيث يمكن لـ Ops تكرارها أو تكرارها.

`scripts/generate-dns-cutover-plan.mjs` يقوم بتكوين واصف ويتم لصقه من `sorafs-pin-release.sh`. للأجيال القادمة:

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

بيانات التعريف الاختيارية المضمنة من خلال الحماية المستمرة من خلال مساعد الدبوس:

| دائم | الاسم |
| --- | --- |
| `DNS_CHANGE_TICKET` | معرف التذكرة، الذي يتم تضمينه في الواصف. |
| `DNS_CUTOVER_WINDOW` | ISO8601 قطع كامل (على سبيل المثال، `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`، `DNS_ZONE` | اسم مضيف الإنتاج + المنطقة المعتمدة. |
| `DNS_OPS_CONTACT` | الاسم المستعار عند الطلب أو جهة اتصال التصعيد. |
| `DNS_CACHE_PURGE_ENDPOINT` | تطهير نقطة النهاية، المذكورة في الواصف. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var с purge token (الافتراضي: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | أدخل إلى واصف القطع المسبق لبيانات تعريف التراجع. |

قم بإجراء مراجعة JSON من خلال مراجعة تغيير DNS، بحيث يمكن للموافقين التحقق من ملخص البيان والارتباطات المستعارة وأوامر التحقيق دون عرض شعار CI. أعلام CLI `--dns-change-ticket`، `--dns-cutover-window`، `--dns-hostname`، `--dns-zone`، `--ops-contact`، `--cache-purge-endpoint`، `--cache-purge-auth-env` و `--previous-dns-plan` يجب أن يتم التجاوز عند فتح المساعد في CI.

## الجزء 8 - تنسيق هيكل ملف Zonefile للمحلل (اختياري)

عند ظهور نافذة تحويل الإنتاج المعروفة، يمكن تعديل البرنامج النصي للإصدار تلقائيًا إلى هيكل SNS Zonefile ومقتطف المحلل. قم بإدخال دفاتر DNS وبيانات التعريف الجديدة عبر env أو CLI؛ يقوم المساعد بإرجاع `scripts/sns_zonefile_skeleton.py` مرة أخرى بعد إنشاء واصف القطع. قم بتنزيل الحد الأدنى من حمولة A/AAAA/CNAME وGAR (BLAKE3-256 حمولة GAR اللاحقة). إذا تم تقديم اسم المنطقة/اسم المضيف و`--dns-zonefile-out`، يرسل المساعد إلى `artifacts/sns/zonefiles/<zone>/<hostname>.json` ويقوم بإنشاء `ops/soradns/static_zones.<hostname>.json` كمقتطف محلل.

| بيريمينايا / علم | الاسم |
| --- | --- |
| `DNS_ZONEFILE_OUT`، `--dns-zonefile-out` | يمكنك الوصول إلى هيكل Zonefile المنسق. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`، `--dns-zonefile-resolver-snippet` | أدخل مقتطف المحلل (من خلال `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`، `--dns-zonefile-ttl` | TTL للكتابة (الافتراضي: 600 ثانية). |
| `DNS_ZONEFILE_IPV4`، `--dns-zonefile-ipv4` | عنوان IPv4 (بيئة مفصولة بفواصل أو علامة لاحقة). |
| `DNS_ZONEFILE_IPV6`، `--dns-zonefile-ipv6` | عنوان IPv6. |
| `DNS_ZONEFILE_CNAME`، `--dns-zonefile-cname` | هدف CNAME الاختياري. |
| `DNS_ZONEFILE_SPKI`، `--dns-zonefile-spki-pin` | دبابيس SHA-256 SPKI (base64). |
| `DNS_ZONEFILE_TXT`، `--dns-zonefile-txt` | اكتب TXT الإضافي (`key=value`). |
| `DNS_ZONEFILE_VERSION`، `--dns-zonefile-version` | تجاوز لتسمية إصدار Zonefile المحسوبة. |
| `DNS_ZONEFILE_EFFECTIVE_AT`، `--dns-zonefile-effective-at` | استخدم الطابع الزمني `effective_at` (RFC3339) في بداية نافذة القطع. |
| `DNS_ZONEFILE_PROOF`، `--dns-zonefile-proof` | تجاوز للإثبات الحرفي في البيانات الوصفية. |
| `DNS_ZONEFILE_CID`، `--dns-zonefile-cid` | تجاوز CID في البيانات الوصفية. |
| `DNS_ZONEFILE_FREEZE_STATE`، `--dns-zonefile-freeze-state` | حالة تجميد الحارس (ناعم، صلب، ذوبان، مراقبة، طوارئ). |
| `DNS_ZONEFILE_FREEZE_TICKET`، `--dns-zonefile-freeze-ticket` | مرجع تذكرة الوصي/المجلس للتجميد. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`، `--dns-zonefile-freeze-expires-at` | RFC3339 الطابع الزمني لإذابة الجليد. |
| `DNS_ZONEFILE_FREEZE_NOTES`، `--dns-zonefile-freeze-note` | ملاحظات التجميد الإضافية (بيئة مفصولة بفواصل أو علم آخر). |
| `DNS_GAR_DIGEST`، `--dns-gar-digest` | BLAKE3-256 ملخص (ست عشري) يدعم حمولة GAR. مطلوب من خلال روابط البوابة. |

تشرح Workflow GitHub Actions ذلك من خلال مستودع الأسرار الذي يؤدي إلى إنتاج دبوس تلقائي يحذف عناصر Zonefile. قم بإدراج الأسرار التالية (يمكن للخطوات دمج قوائم مفصولة بفواصل لشريط متعدد القيم):

| سر | الاسم |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`، `DOCS_SORAFS_DNS_ZONE` | اسم المضيف/منطقة الإنتاج لمساعد. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | الاسم المستعار عند الطلب в الواصف. |
| `DOCS_SORAFS_ZONEFILE_IPV4`، `DOCS_SORAFS_ZONEFILE_IPV6` | IPv4/IPv6 سجل للنشر. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | هدف CNAME الاختياري. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | قاعدة دبابيس SPKI64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | قم بتسجيل TXT الإضافي. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | تجميد البيانات الوصفية في الهيكل العظمي. |
| `DOCS_SORAFS_GAR_DIGEST` | Hex BLAKE3 يهضم الحمولة الصافية GAR. |

عند البدء في `.github/workflows/docs-portal-sorafs-pin.yml`، قم بتمكين المدخلات `dns_change_ticket` و`dns_cutover_window`، لحذف الواصف/ملف المنطقة بشكل صحيح. اترك هذه الوسادات للتشغيل الجاف فقط.

الأمر النموذجي (لمالك دفتر التشغيل SN-7):

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
  ...other flags...
```

يقوم المساعد تلقائيًا بإعادة تغيير التذكرة في TXT، ثم قم بكتابة أول نافذة قطع مثل `effective_at`، إذا لم يتم تثبيتها. سير العمل التشغيلي الشامل sm. في `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### دعم مفوضي DNS العام

يوفر ملف المنطقة الهيكلية مساحة تخزينية ذات صلاحية محدودة فقط. مندوبي NS/DS
يجب إنشاء منطقة أصحاب الأرض في المسجل أو موفر DNS لذلك
يمكن للإنترنت العادي العثور على خادم الأسماء الخاص بك.

- لقطع القمة/TLD، استخدم ALIAS/ANAME (اطلع على المقدم) أو
  قم بنشر صفحة A/AAAA، التي تدعم بوابة Anycast-IP.
- للمساعدة في نشر CNAME للمضيف الجميل المشتق
  (`<fqdn>.gw.sora.name`).
- المضيف الكنسي (`<hash>.gw.sora.id`) موجود في بوابة المجال ولا يوجد
  يتم نشرها في منطقتك العامة.

### بوابة شابلون زاغولوفكوف

نشر المساعد أيضًا الذي أنشأ `portal.gateway.headers.txt` و`portal.gateway.binding.json` - عنصران مبتكران DG-3 مطلوبان لربط محتوى البوابة:

- يقوم `portal.gateway.headers.txt` بتوصيل كتلة كبيرة من رؤوس HTTP (بما في ذلك `Sora-Name`، و`Sora-Content-CID`، و`Sora-Proof`، وCSP، وHSTS، و`Sora-Route-Binding`)، والتي يتم لصق بوابة الحافة من خلال فتحة طويلة.
- يقوم `portal.gateway.binding.json` بإصلاحها ومعلوماتها من خلال الآلة لتغيير التذاكر ويمكن للأتمتة أن تتعرف على روابط المضيف/CID بدون تحليل قذيفة вывода.

يتم إنشاء هذا من خلال `cargo xtask soradns-binding-template` ويثبت الاسم المستعار وملخص البيان واسم مضيف البوابة، والتي تم تغييرها في `sorafs-pin-release.sh`. للتجديد أو لتخصيص رؤوس الكتل:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

قم بإعداد `--csp-template`، أو `--permissions-template`، أو `--hsts-template`، لإعادة تقديم شيبلونز زاجولوفكوف؛ استخدمها الآن مع `--no-*` الأعلام للرأس الكامل.

قم بإدراج مقتطف من طلب تغيير CDN وإعادة تشغيل JSON في خط أنابيب أتمتة البوابة، مما يؤدي إلى إنتاج حقيقي لهذه الأدلة الرائعة.

قم بتحرير البرنامج النصي تلقائيًا لمساعد التحقق من أجل الحصول على تذاكر DG-3 بعد تجميع جميع الأدلة. قم بالتوصيل أولاً، إذا قمت بربط JSON أولاً:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

أمر فك التشفير `Sora-Proof`، تحقق من `Sora-Route-Binding` باستخدام ملف CID الظاهر + اسم المضيف والاتصال بالرؤوس الطرفية. قم بأرشفة الإصدار مباشرة باستخدام عناصر الإصدار الأخرى عند بدء CI، حيث يقوم مراجعو DG-3 بإرسال عمليات التحقق من المصادقة.

> ** واصف DNS للتكامل:** يتضمن الخط `portal.dns-cutover.json` القسم `gateway_binding`، الذي يوضح هذه العناصر (المفتاح، المحتوى، دليل الحالة وقالب الرأس الحرفي) ** و** القسم `route_plan`، متصل بـ `gateway.route_plan.json` والقالب الرئيسي/الاستعادة. قم بإدراج هذه الكتل في تذكرة تغيير DG-3 حتى يتمكن المراجعون من التعرف على أهم المعلومات `Sora-Name/Sora-Proof/CSP` وإعادة صياغة الخطة المرغوبة حزمة أدلة الترويج/التراجع دون فتح أرشيف البناء.

## الجزء 9 - تثبيت شاشات النشرعنصر خريطة الطريق **DOCS-3c** يحتاج إلى مزيد من الدلالة على أن البوابة، جربها الوكيل وروابط البوابة تتخلص من العيوب بعد الإصدار. قم بتشغيل جهاز المراقبة الموحد مؤقتًا بعد 7-8 وقم بضمه إلى تحقيقاتك المجدولة:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` قم بحفظ التكوين (على سبيل المثال `docs/portal/docs/devportal/publishing-monitoring.md` للأنظمة) واستخدم ثلاثة تحقيقات: تحقيقات بوابة البوابة + التحقق من صحة سياسة CSP/Permissions، جرب تحقيقات الوكيل (اختياريًا `/metrics`)، وأداة التحقق من ربط البوابة (`cargo xtask soradns-verify-binding`)، التي تحتاج إلى الحصول على الاسم والاسم المستعار Sora-Content-CID بالاسم المستعار/البيان التحقق.
- يتم ضبط الأمر على قيمة غير صفرية، عندما يبدأ أي مسبار في العمل، حيث يمكن لمشغلي CI/cron/runbook الاستمرار في الاعتماد على الاسم المستعار للمنتج.
- علم `--json-out` أرسل ملخصًا واحدًا JSON по елям; `--evidence-dir` أرسل `summary.json` و`portal.json` و`tryit.json` و`binding.json` و`checksums.sha256` لكي يتمكن مراجعو الإدارة من المناقشة نتائج دون مراقبة لاحقة. قم بأرشفة هذا الكتالوج تحت `artifacts/sorafs/<tag>/monitoring/` باستخدام حزمة Sigstore واصف تحويل DNS.
- قم بتضمين مراقبة البث وتصدير Grafana (`dashboards/grafana/docs_portal.json`) ومعرف حفر Alertmanager في تذكرة الإصدار، بحيث يمكن الاستماع إلى DOCS-3c SLO. تم نشر نشرة مراقبة قواعد اللعبة التي تمارسها في `docs/portal/docs/devportal/publishing-monitoring.md`.

تحتاج تحقيقات البوابة إلى استخدام HTTPS وإلغاء حجب عنوان URL الأساسي لـ `http://`، إذا لم يتم تثبيت `allowInsecureHttp` في شاشة التكوين؛ قم بالاحتفاظ بالإنتاج/العرض المسرحي على TLS وقم بإدراج التجاوز فقط للمعاينة المحلية.

قم بأتمتة الشاشة عبر `npm run monitor:publishing` في Buildkite/cron بعد فتح البوابة. بالإضافة إلى الأمر، الذي يتم ضبطه على عناوين URL الخاصة بالإنتاج، فإنه يتضمن فحوصات السلامة اللاحقة بين الإصدارات.

## الأتمتة من خلال `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` متضمن 2-6. على:

1. أرشفة `build/` في تحديد كرة القطران،
2. أغلق `car pack`، و`manifest build`، و`manifest sign`، و`manifest verify-signature`، و`proof verify`،
3. اختياريًا، قم باختيار `manifest submit` (بما في ذلك ربط الاسم المستعار) عند الحصول على بيانات اعتماد Torii، و
4. أدخل `artifacts/sorafs/portal.pin.report.json`، `portal.pin.proposal.json` الاختياري، واصف قطع DNS (بعد التقديم) وحزمة ربط البوابة (`portal.gateway.binding.json` بالإضافة إلى كتلة الرأس)، لأوامر الإدارة/الشبكات/عمليات يمكنك التحقق من الأدلة دون الحصول على شعار CI.

قبل التثبيت، قم بالتثبيت `PIN_ALIAS` و`PIN_ALIAS_NAMESPACE` و`PIN_ALIAS_NAME` و(اختياريًا) `PIN_ALIAS_PROOF_PATH`. استخدم `--skip-submit` للتشغيل الجاف؛ لا يمكن إلغاء سير عمل GitHub من خلال الإدخال `perform_submit`.

## الجزء 8 - مواصفات النشر OpenAPI وحزم SBOM

يحتاج DOCS-7 إلى بوابة ومواصفات OpenAPI وعناصر SBOM التي تنتج واحدًا وهو خط أنابيب محدد. مساعدين مميزين يظهرون كل ثلاثة:

1. **إعادة إنشاء المواصفات ونشرها.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   استخدم `--version=<label>` لحفظ اللقطة التاريخية (على سبيل المثال `2025-q3`). يرسل المساعد لقطة إلى `static/openapi/versions/<label>/torii.json`، ويسجل في `versions/current` ويسجل بيانات التعريف (SHA-256، وحالة البيان، والطابع الزمني المحدث) في `static/openapi/versions.json`. تستخدم البوابة هذا الفهرس لاختيار النسخة والموافقة على الملخص/التوقيع. إذا لم يكن `--version` مقيدًا، فسيتم تضمين التسميات المفيدة ويستبدل فقط `current` + `latest`.

   يقوم Manifest بإصلاح ملخصات SHA-256/BLAKE3، بحيث يمكن للبوابة تثبيت `Sora-Proof` لـ `/reference/torii-swagger`.

2. ** قم بتكوين CycloneDX SBOMs. ** يمكن أن يساعد إصدار خط الأنابيب على SBOMs المستندة إلى syft المتوافقة مع `docs/source/sorafs_release_pipeline_plan.md`. أحدث طريقة لبناء القطع الأثرية:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **قم بتحميل كل الحمولة في السيارة.**

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

   اتبع هذا الموضوع `manifest build`/`manifest sign`، وهو ما يعني للموقع الرئيسي، الاسم المستعار النهائي للمنتج (على سبيل المثال، `docs-openapi.sora` للمواصفات و`docs-sbom.sora` للمواصفات SBOM). Отдельные alias оставляют SoraDNS, GAR وتذاكر التراجع قابلة للتخصيص من الحمولة المحددة.

4. **إرسال وربط.** استخدم السلطة وحزمة Sigstore، ولكن قم بإصلاح الاسم المستعار في قائمة التحقق من الإصدار، ليعرف المدققون ما هو اسم Sora сооответствует какому الهضم.

تتوفر مواصفات الأرشفة/بيانات SBOM مع بوابة البناء التي تضمن أن كل تذكرة إصدار تقوم بالتواصل مع مجموعة كاملة من المصنوعات بدون إغلاق لاحق باكر.

### مساعد الأتمتة (CI/البرنامج النصي للحزمة)

`./ci/package_docs_portal_sorafs.sh` يقوم بالترميز من 1 إلى 8، بحيث يمكن استخدام عنصر خريطة الطريق **DOCS-7** من أمر واحد. مساعد:

- استخدام بوابة الدخول (`npm ci`، OpenAPI/Norito، المزامنة، اختبارات القطعة)؛
- قم بتكوين سيارات وأزواج بيان للبوابة، OpenAPI وSBOM من خلال `sorafs_cli`؛
- اختياريًا، قم باختيار `sorafs_cli proof verify` (`--proof`) وتوقيع Sigstore (`--sign`، `--sigstore-provider`، `--sigstore-audience`)؛
- قم بربط جميع المصنوعات اليدوية في `artifacts/devportal/sorafs/<timestamp>/` وأدخل `package_summary.json` لأدوات CI/الإصدار؛ و
- قم بتغيير `artifacts/devportal/sorafs/latest` إلى الختم التالي.

مثال (خط أنابيب كامل مع Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

الأعلام البوليسية:

- `--out <dir>` - تجاوز المصنوعات اليدوية (من خلال كتالوجات الترقية ذات الطابع الزمني).
- `--skip-build` - использовать существующий `docs/portal/build` (المرايا غير المتصلة بالإنترنت متاحة للاستخدام).
- `--skip-sync-openapi` - قم بإنشاء `npm run sync-openapi`، عندما لا يمكن توصيل `cargo xtask openapi` إلى صناديق.io.
- `--skip-sbom` - لا تحدد `syft`، إذا لم يكن الثنائي مستقرًا (النص البرمجي يقطع مسبقًا).
- `--proof` - قم باختيار `sorafs_cli proof verify` لكل سيارة/بيان. الحمولات متعددة الملفات التي تحتاج إلى دعم خطة القطع في CLI، قم بإلغاء تحديد هذه العلامة المميزة في `plan chunk count` و تحقق مرة أخرى من بوابة التحكم.
- `--sign` - قم بالاتصال بـ `sorafs_cli manifest sign`. قم بإدخال الرمز المميز من خلال `SIGSTORE_ID_TOKEN` (أو `--sigstore-token-env`)، مما يسمح لـ CLI بالحصول عليه من خلال `--sigstore-provider/--sigstore-audience`.

لإنتاج القطع الأثرية استخدم `docs/portal/scripts/sorafs-pin-release.sh`. على بوابة الحزمة، OpenAPI وSBOM، يسجل كل بيان ويسجل بيانات التعريف الإضافية في `portal.additional_assets.json`. يشير المساعد إلى المقابض وحزم CI، بالإضافة إلى `--openapi-*` و`--portal-sbom-*` و`--openapi-sbom-*` للإعدادات المستعارة tuples، وتجاوز مصدر SBOM من خلال `--openapi-sbom-source`، الحمولات الإضافية (`--skip-openapi`/`--skip-sbom`) والتعيين غير القياسي `syft` عبر `--syft-bin`.

البرنامج النصي يصدر جميع الأوامر؛ انسخ السجل في تذكرة الإصدار باستخدام `package_summary.json` حتى يتمكن المراجعون من التحقق من ملخصات CAR وبيانات تعريف الخطة وتجزئة حزمة Sigstore دون أي مساعدة قذيفة вывода.

## الجزء 9 - بوابة التحقق + SoraDNS

قبل أن نلاحظ أن الاسم المستعار الجديد تم حله عبر SoraDNS والبوابات التي تقدم البراهين المختلفة:

1. **أغلق بوابة المسبار.** `ci/check_sorafs_gateway_probe.sh` يتم استخدام `cargo xtask sorafs-gateway-probe` على التركيبات التجريبية في `fixtures/sorafs_gateway/probe_demo/`. من أجل النشر الحقيقي، يمكنك العثور على اسم المضيف المستهدف:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   قم بفحص فك ترميز `Sora-Name` و`Sora-Proof` و`Sora-Proof-Status` إلى `docs/source/sorafs_alias_policy.md` والاتصال ببيانات الملخص المرسلة أو روابط TTL أو GAR.

   لإجراء فحوصات مفاجئة خفيفة الوزن (على سبيل المثال، عند ربط الحزمة فقط
   تم التغيير)، قم بتشغيل `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   يقوم المساعد بالتحقق من صحة حزمة الربط التي تم التقاطها وهو سهل الإصدار
   التذاكر التي تحتاج فقط إلى تأكيد ملزم بدلاً من تدريبات التحقيق الكاملة.

2. **الحصول على الأدلة للحفر.** لتدريبات المشغل أو عمليات التشغيل الجافة PagerDuty، استخدم `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...`. يقوم المجمع بتخزين الرؤوس/السجلات في `artifacts/sorafs_gateway_probe/<stamp>/`، ويعيد `ops/drill-log.md` ويطلق بشكل اختياري خطافات التراجع أو حمولات PagerDuty. قم بتثبيت `--host docs.sora` للتحقق من أن مسار SoraDNS ليس IP.

3. **التحقق من ارتباطات DNS.** عند نشر دليل الاسم المستعار للحوكمة، ودمج ملف GAR، وتسجيله في التحقيق (`--gar`)، وقصه لإصدار الأدلة. يمكن لمالكي المحلل أن يتنبأوا بذلك من خلال `tools/soradns-resolver` للاستماع إلى ما يكتبونه من بيان جديد. قبل تطبيق JSON، تم استخدام `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]` لتحديد تعيين المضيف وبيانات التعريف وتسميات القياس عن بعد دون الاتصال بالإنترنت. يمكن للمساعد تنسيق ملخص `--json-out` مع GAR الموضح.
  عند إنشاء GAR الجديد، استخدم `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...` (يتم الاتصال به إلى `--manifest-cid <cid>` فقط عند تقديم ملف البيان). مساعد тепerь выводит CID **и** BLAKE3 ملخص أول من البيان JSON، إجراء الاختبارات، إلغاء تكرار `--telemetry-label`، فرز التسميات و قم بإرسال قوالب CSP/HSTS/Permissions-Policy الافتراضية قبل إنشاء JSON، لتتمكن من تحديد الحمولة النافعة.

4. **الرجوع إلى الاسم المستعار للقياس.** انتقل إلى الشاشة `torii_sorafs_alias_cache_refresh_duration_ms` و`torii_sorafs_gateway_refusals_total{profile="docs"}`; السلسلة موجودة في `dashboards/grafana/docs_portal.json`.

## الجزء 10 - المراقبة وتجميع الأدلة- **لوحات المعلومات.** قم بتصدير `dashboards/grafana/docs_portal.json` (بوابة SLO)، و`dashboards/grafana/sorafs_gateway_observability.json` (بوابة زمن الاستجابة + صحة الإثبات) و`dashboards/grafana/sorafs_fetch_observability.json` (سلامة المنسق) للإصدار التالي. قم بإلغاء تصدير JSON إلى تذكرة الإصدار.
- **أرشيفات التحقيق.** اطلع على `artifacts/sorafs_gateway_probe/<stamp>/` في git-annex أو مجموعة الأدلة. قم بإلغاء تحديد ملخص التحقيق والرؤوس وحمولة PagerDuty.
- **حزمة الإصدار.** قم بدعم مدخل ملخص CAR/SBOM/OpenAPI، وحزم البيانات، وتوقيعات Sigstore، و`portal.pin.report.json`، وسجلات اختبار Try-It، وتقارير التحقق من الارتباط في حزمة واحدة (على سبيل المثال، `artifacts/sorafs/devportal/20260212T1103Z/`).
- **سجل الحفر.** عند تحقيقات - جزء من الحفر، يتم توصيل `scripts/telemetry/run_sorafs_gateway_probe.sh` إلى `ops/drill-log.md`، مما يؤدي إلى تحقيق متطلبات الفوضى SNNet-5.
- **روابط التذاكر.** يمكنك الحصول على معرفات اللوحة Grafana أو صادرات PNG المحددة في تغيير التذكرة باستخدام تقرير التحقيق، بحيث يمكن للمراجعين التحقق من SLOs بدون الصدفة التسليم.

## الجزء 11 - حفر جلب متعدد المصادر وأدلة لوحة النتائج

النشر في SoraFS يحتاج الآن إلى أدلة جلب متعددة المصادر (DOCS-7/SF-6) مع إثباتات DNS/البوابة. بيان الدبوس التالي:

1. **قم بتثبيت `sorafs_fetch` على البيان المباشر.** استخدم أيضًا التخطيط/البيانات المصطنعة من العناصر 2-3 وبيانات اعتماد البوابة لأي سبب من الأسباب. قم بتزويد جميع البيانات الضرورية حتى يتمكن المدققون من اتخاذ قرار بشأن المنسق:

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

   - ابدأ بالحصول على إعلانات الموفر، الموضحة في البيان (على سبيل المثال، `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) وأتبعها من خلال `--provider-advert name=path`، وذلك من أجل لوحة النتائج الرئيسية القدرة على تحديد كل شيء. `--allow-implicit-provider-metadata` يستخدم **فقط** لتركيبات CI; يجب أن يتم تدريبات الإنتاج من خلال الإعلانات اللاحقة من دبوس.
   - عند الانتهاء من المناطق الإضافية، قم بتوجيه الأوامر باستخدام مجموعات الموفر المفضلة، من أجل جلب ذاكرة التخزين المؤقت/الاسم المستعار للاسم المستعار قطعة أثرية.

2. **أرشفة البيانات.** قم بالتسجيل `scoreboard.json` و`providers.ndjson` و`fetch.json` و`chunk_receipts.ndjson` في نسخة كتالوج الأدلة. تقوم هذه الملفات بإصلاح ترجيح الأقران وميزانية إعادة المحاولة ووقت استجابة EWMA وإيصالات القطع لـ SF-7.

3. **استخدام القياس عن بعد.** استيراد مخرجات الجلب من لوحة المعلومات **SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`)، تسلسلات `torii_sorafs_fetch_duration_ms`/`_failures_total` و لوحات على نطاقات الموفر. قم بعرض لقطات Grafana لتذكرة الإصدار باستخدام لوحة النتائج.

4. **التحقق من قواعد التنبيه.** قم بتثبيت `scripts/telemetry/test_sorafs_fetch_alerts.sh` للتحقق من حزمة التنبيه Prometheus لإصدار الحماية. قم بقص إخراج promtool، حيث أكد مراجعو DOCS-7 أن تنبيهات الموفر البطيئة/المماطلة نشطة.

5. **إدخال في CI.** مسار عمل دبوس البوابة `sorafs_fetch` جزء من الإدخال `perform_fetch_probe`; تضمين الذات في التدريج/الإنتاج لجلب الأدلة التي تم تشكيلها باستخدام حزمة البيان. يمكن أن تستخدم التدريبات المحلية كلاً من النص البرمجي ورموز بوابة التصدير والمصدر `PIN_FETCH_PROVIDERS` (موفر القائمة المفصولة بفواصل).

## الترويج وإمكانية الملاحظة والتراجع

1. **الترويج:** قم بالترقية إلى الاسم المستعار للإنتاج والإنتاج. قم بالبدء بالبدء مرة أخرى في `manifest submit` بموضوع البيان/الحزمة، من خلال `--alias-namespace/--alias-name` للاسم المستعار للإنتاج. هذا يشمل النقل/النقل بعد الموافقة على ضمان الجودة.
2. **المراقبة:** قم بملء سجل دبوس لوحة القيادة (`docs/source/grafana_sorafs_pin_registry.json`) وتحقيقات البوابة (`docs/portal/docs/devportal/observability.md`). خطوات الانجراف الاختباري أو المسابر الفاشلة أو إثبات إعادة المحاولة.
3. **التراجع:** للعودة مرة أخرى إلى البيان السابق (أو سحب الاسم المستعار المؤقت) باستخدام `sorafs_cli manifest submit --alias ... --retire`. في المرة القادمة، الحزمة السابقة المعروفة وملخص CAR، يمكن أن تكون بروفات التراجع قابلة للتنفيذ عند تدوير سجلات CI.

## سير عمل شابلون CI

الحد الأدنى من طول خط الأنابيب:

1. البناء + الوبر (`npm ci`، `npm run build`، المجموع الاختباري للإنشاء).
2. التعبئة (`car pack`) وبيان التعبئة.
3. أدخل الرمز المميز OIDC الخاص بنطاق العمل (`manifest sign`).
4. المصنوعات اليدوية (السيارة، البيان، الحزمة، الخطة، الملخصات) للتدقيق.
5. تصحيح رقم التعريف الشخصي:
   - سحب الطلبات -> `docs-preview.sora`.
   - العلامات / الفروع المحمية -> الاسم المستعار للإنتاج الترويجي.
6. تحقيقات التشغيل + بوابات التحقق من الإثبات قبل الانتهاء.

`.github/workflows/docs-portal-sorafs-pin.yml` ينضم إلى هذه الأشياء للإصدارات اليدوية. سير العمل:

- بوابة البناء/الاختبار،
- بناء التعبئة عبر `scripts/sorafs-pin-release.sh`،
- نشر/التحقق من حزمة البيان عبر GitHub OIDC،
- تأمين السيارة/البيان/الحزمة/الخطة/ملخصات الإثبات مثل القطع الأثرية، وما إلى ذلك
- (اختياريًا) تنفيذ البيان + ربط الاسم المستعار عند الحصول على الأسرار.

قم بإدراج أسرار/متغيرات المستودع التالية قبل بدء المهمة:

| الاسم | الاسم |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | مضيف Torii مع `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | معرف العصر، يتم كتابته عند الإرسال. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | السلطة المصاحبة لتقديم البيان. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | الاسم المستعار صف، خاص عند `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | حزمة إثبات الاسم المستعار Base64 (اختياري). |
| `DOCS_ANALYTICS_*` | التحليلات الداخلية/استكشاف نقاط النهاية. |

ابدأ سير العمل من خلال واجهة مستخدم الإجراءات:

1. حدد `alias_label` (على سبيل المثال، `docs.sora.link`)، و`proposal_alias` الاختياري، و`release_tag` الاختياري.
2. تثبيت `perform_submit` حصريًا للإنشاءات المصنوعة بخلاف Torii (التشغيل التجريبي) أو تضمين عناصر النشر على الاسم المستعار настроенный.

`docs/source/sorafs_ci_templates.md` يصف مساعدي CI السابقين للمشاريع الأخرى، ولكن للاستخدام التالي لاستخدام سير عمل البوابة.

## قائمة الاختيار

- [ ] يتم عرض `npm run build` و`npm run test:*` و`npm run check:links`.
- [ ] تم تخزين `build/checksums.sha256` و`build/release.json` في القطع الأثرية.
- [ ] السيارة والخطة والبيان والملخص الذي تم إعداده تحت `artifacts/`.
- [ ] حزمة Sigstore + التوقيع المنفصل المرتبط بالسجلات.
- [ ] يتم تخزين `portal.manifest.submit.summary.json` و`portal.manifest.submit.response.json` عند الإرسال.
- [ ] `portal.pin.report.json` (و`portal.pin.proposal.json` اختياريًا) يتم أرشفتها باستخدام CAR/التحف الظاهرة.
- [ ] الشعاران `proof verify` و`manifest verify-signature` محميان.
- [ ] تم تحديث لوحات المعلومات Grafana + تحقيقات Try-It ناجحة.
- [ ] ملاحظات التراجع (البيان السابق للمعرف + خلاصة الاسم المستعار) متاحة من خلال تذكرة الإصدار.