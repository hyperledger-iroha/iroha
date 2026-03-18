---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/devportal/deploy-guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1a8779f125dc666e735e514ca472f36bbbbcdd7336f53bb9e57846266c0bf5f7
source_last_modified: "2026-01-22T15:57:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: deploy-guide
lang: ar
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

## نظرة عامة

يحولهذا الدليل الاجرائي عناصر خارطة الطريق **DOCS-7** (نشر SoraFS) و **DOCS-8** (اتمتة pin في CI/CD) الى اجراء عملي لبوابة المطورين. يغطي مرحلة build/lint، تغليف SoraFS، توقيع المانيفست عبر Sigstore، ترقية الاسماء المستعارة، التحقق، وتمارين التراجع بحيث تكون كل معاينة واصدار قابلة لاعادة الانتاج وقابلة للتدقيق.

يفترض هذا المسار ان لديك ثنائي `sorafs_cli` (مبني عبر `--features cli`)، وصولا الى Torii endpoint مع صلاحيات pin-registry، وبيانات اعتماد OIDC لـ Sigstore. خزّن الاسرار طويلة العمر (`IROHA_PRIVATE_KEY`، `SIGSTORE_ID_TOKEN`، رموز Torii) في خزينة CI؛ ويمكن للعمليات المحلية تحميلها عبر exports من الـ shell.

## المتطلبات المسبقة

- Node 18.18 او احدث مع `npm` او `pnpm`.
- `sorafs_cli` من `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- عنوان Torii يكشف `/v1/sorafs/*` مع حساب/مفتاح خاص بسلطة يمكنها ارسال المانيفست والاسماء المستعارة.
- مُصدِر OIDC (GitHub Actions، GitLab، workload identity، الخ) لاصدار `SIGSTORE_ID_TOKEN`.
- اختياري: `examples/sorafs_cli_quickstart.sh` للتجارب الجافة و `docs/source/sorafs_ci_templates.md` لقوالب workflows في GitHub/GitLab.
- اضبط متغيرات OAuth الخاصة بـ Try it (`DOCS_OAUTH_*`) وشغّل
  [security-hardening checklist](./security-hardening.md) قبل ترقية build خارج المختبر. بناء البوابة يفشل الان عند غياب هذه المتغيرات او عند خروج مفاتيح TTL/polling عن النوافذ المفروضة؛ استخدم `DOCS_OAUTH_ALLOW_INSECURE=1` فقط لمعاينات محلية مؤقتة. ارفق ادلة اختبار الاختراق مع تذكرة الاصدار.

## الخطوة 0 - التقاط حزمة وكيل Try it

قبل ترقية المعاينة الى Netlify او البوابة، ثبّت مصادر وكيل Try it وبصمة المانيفست OpenAPI الموقّع في حزمة حتمية:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

يقوم `scripts/tryit-proxy-release.mjs` بنسخ ادوات proxy/probe/rollback، والتحقق من توقيع OpenAPI وكتابة `release.json` مع `checksums.sha256`. ارفق هذه الحزمة مع تذكرة ترقية Netlify/SoraFS gateway كي يتمكن المراجعون من اعادة تشغيل مصادر الوكيل الدقيقة وتلميحات Torii بدون اعادة بناء. تسجّل الحزمة ايضا ما اذا كان تم تمكين bearer tokens المرسلة من العميل (`allow_client_auth`) للحفاظ على اتساق خطة الاطلاق وقواعد CSP.

## الخطوة 1 - بناء وفحص lint للبوابة

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

ينفذ `npm run build` تلقائيا `scripts/write-checksums.mjs` وينتج:

- `build/checksums.sha256` - مانيفست SHA256 صالح لـ `sha256sum -c`.
- `build/release.json` - بيانات وصفية (`tag`، `generated_at`، `source`) مثبتة في كل CAR/مانيفست.

احفظ كلا الملفين مع ملخص CAR حتى يتمكن المراجعون من مقارنة نواتج المعاينة بدون اعادة بناء.

## الخطوة 2 - تغليف الاصول الثابتة

شغّل CAR packer مقابل مخرجات Docusaurus. المثال ادناه يكتب كل الاصول تحت `artifacts/devportal/`.

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

يلتقط ملخص JSON عدد الـ chunks، والـ digests، وتلميحات تخطيط proof التي يعيد `manifest build` ولوحات CI استخدامها لاحقا.

## الخطوة 2b - تغليف مرافقات OpenAPI و SBOM

يتطلب DOCS-7 نشر موقع البوابة ولقطة OpenAPI و SBOM كمانيفستات منفصلة بحيث تتمكن البوابات من تدبيس رؤوس `Sora-Proof`/`Sora-Content-CID` لكل artefact. يقوم مساعد الاصدار (`scripts/sorafs-pin-release.sh`) بالفعل بتغليف مجلد OpenAPI (`static/openapi/`) وملفات SBOM الناتجة عبر `syft` في CARs منفصلة `openapi.*`/`*-sbom.*` ويسجل البيانات في `artifacts/sorafs/portal.additional_assets.json`. عند اتباع المسار اليدوي، اعد الخطوات 2-4 لكل payload مع بادئاتها ووسوم metadata الخاصة بها (مثلا `--car-out "$OUT"/openapi.car` مع `--metadata alias_label=docs.sora.link/openapi`). سجّل كل زوج manifest/alias في Torii (الموقع، OpenAPI، SBOM البوابة، SBOM OpenAPI) قبل تبديل DNS حتى تتمكن البوابة من تقديم proofs مدبسة لكل artefact منشور.

## الخطوة 3 - بناء المانيفست

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

عدّل اعلام pin-policy بما يتناسب مع نافذة الاصدار (مثلا `--pin-storage-class hot` للكاناري). نسخة JSON اختيارية لكنها مفيدة للمراجعة.

## الخطوة 4 - التوقيع عبر Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

يسجل الـ bundle بصمة المانيفست وبصمات الـ chunks وهاش BLAKE3 للـ OIDC token دون حفظ JWT. احتفظ بالـ bundle والتوقيع المنفصل؛ يمكن لعمليات الترقية في الانتاج اعادة استخدام نفس الاصول بدون اعادة توقيع. يمكن للعمليات المحلية استبدال اعلام المزود بـ `--identity-token-env` (او تعيين `SIGSTORE_ID_TOKEN` في البيئة) عندما يصدر مساعد OIDC خارجي التوكن.

## الخطوة 5 - الارسال الى pin registry

ارسل المانيفست الموقّع (وخطة الـ chunks) الى Torii. اطلب دائما summary كي تبقى نتيجة التسجيل/الاسم المستعار قابلة للتدقيق.

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

عند اطلاق معاينة او كاناري (`docs-preview.sora`)، اعد الارسال باستخدام alias مختلف حتى يتمكن QA من التحقق قبل ترقية الانتاج.

يتطلب ربط الاسم المستعار ثلاثة حقول: `--alias-namespace` و `--alias-name` و `--alias-proof`. ينتج فريق الحوكمة bundle proof (base64 او Norito bytes) عند الموافقة؛ خزنه ضمن اسرار CI وقدمه كملف قبل استدعاء `manifest submit`. اترك اعلام alias فارغة اذا كنت تريد تثبيت المانيفست بدون تغيير DNS.

## الخطوة 5b - توليد مقترح حوكمة

يجب ان يرافق كل مانيفست مقترح جاهز للبرلمان بحيث يستطيع اي مواطن من Sora تقديم التغيير دون امتلاك بيانات اعتماد مميزة. بعد خطوات submit/sign نفّذ:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

يلتقط `portal.pin.proposal.json` التعليمة القياسية `RegisterPinManifest` وبصمة الـ chunks والسياسة وتلميح alias. ارفقه مع تذكرة الحوكمة او بوابة البرلمان كي يتمكن المندوبون من مراجعة الـ payload دون اعادة بناء. هذا الامر لا يلمس مفتاح Torii، لذلك يمكن لاي مواطن ان ينشئ المقترح محليا.

## الخطوة 6 - التحقق من proofs والقياسات

بعد التثبيت، نفذ خطوات التحقق الحتمية:

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

- راقب `torii_sorafs_gateway_refusals_total` و `torii_sorafs_replication_sla_total{outcome="missed"}` لاكتشاف الشذوذ.
- شغّل `npm run probe:portal` لاختبار وكيل Try-It وروابط التحقق على المحتوى المثبت.
- اجمع ادلة المراقبة المذكورة في
  [Publishing & Monitoring](./publishing-monitoring.md) لتلبية بوابة الملاحظة DOCS-3c. يدعم المساعد الان عدة `bindings` (الموقع، OpenAPI، SBOM للبوابة، SBOM لـ OpenAPI) ويفرض `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` على المضيف المستهدف عبر حارس `hostname`. الاستدعاء ادناه يكتب ملخص JSON واحدا وحزمة الادلة (`portal.json`، `tryit.json`، `binding.json`، `checksums.sha256`) تحت مجلد الاصدار:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## الخطوة 6a - تخطيط شهادات البوابة

استخلص خطة TLS SAN/challenge قبل توليد حزم GAR حتى يراجع فريق البوابة وموافقو DNS نفس الادلة. يعكس المساعد الجديد مدخلات DG-3 عبر تعداد المضيفين القانونيين wildcard و SAN للـ pretty-host وتسميات DNS-01 والتحديات الموصى بها:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

قم بضم JSON مع حزمة الاصدار (او ارفقه مع تذكرة التغيير) حتى يستطيع المشغلون لصق قيم SAN داخل اعدادات `torii.sorafs_gateway.acme` ويتأكد مراجعو GAR من الخرائط canonical/pretty دون اعادة الحساب. اضف `--name` لكل لاحقة اضافية يتم ترقيتها في الاصدار نفسه.

## الخطوة 6b - اشتقاق خرائط المضيفين القانونية

قبل توليد قوالب GAR، سجّل خريطة المضيف الحتمية لكل alias. يقوم `cargo xtask soradns-hosts` بتهشئة كل `--name` الى التسمية القانونية (`<base32>.gw.sora.id`) ويصدر wildcard المطلوب (`*.gw.sora.id`) ويشتق المضيف الجميل (`<alias>.gw.sora.name`). احفظ الناتج ضمن artefacts الاصدار كي يتمكن مراجعو DG-3 من مقارنة الخريطة مع ارسال GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

استخدم `--verify-host-patterns <file>` للفشل السريع عندما يغيب احد المضيفين المطلوبين من GAR او JSON الخاص بالربط. يقبل المساعد عدة ملفات للتحقق، مما يسهل تدقيق قالب GAR و `portal.gateway.binding.json` في نفس الاستدعاء:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

ارفق ملخص JSON وسجل التحقق مع تذكرة تغيير DNS/البوابة حتى يستطيع المدققون تأكيد المضيفين القانونية/الجميلة دون اعادة تشغيل السكربتات. اعد تشغيل الامر عند اضافة alias جديد حتى ترث تحديثات GAR نفس الادلة.

## الخطوة 7 - توليد واصف DNS cutover

تتطلب عمليات القطع في الانتاج حزمة تغيير قابلة للتدقيق. بعد نجاح الارسال (ربط الاسم المستعار)، يصدر المساعد
`artifacts/sorafs/portal.dns-cutover.json`، ويتضمن:

- بيانات ربط الاسم المستعار (namespace/name/proof، بصمة المانيفست، عنوان Torii، epoch الارسال، السلطة).
- سياق الاصدار (tag، alias label، مسارات المانيفست/CAR، خطة الـ chunks، Sigstore bundle).
- مؤشرات التحقق (امر probe، alias + Torii endpoint).
- حقول تغيير اختيارية (معرف التذكرة، نافذة cutover، جهة الاتصال التشغيلية، مضيف/منطقة الانتاج).
- بيانات ترقية المسار المشتقة من رأس `Sora-Route-Binding` (المضيف القانوني/CID، مسارات الرأس + الربط، اوامر التحقق) لضمان ان ترقية GAR وتمارين التراجع تشير لنفس الادلة.
- ملفات route-plan المولدة (`gateway.route_plan.json`، قوالب الرؤوس، ورؤوس التراجع الاختيارية) حتى تتحقق تذاكر DG-3 و lint في CI من خطط الترقية/التراجع.
- بيانات اختيارية لالغاء الكاش (نقطة purge، متغير auth، حمولة JSON، ومثال `curl`).
- تلميحات التراجع التي تشير الى الوصف السابق (وسم الاصدار وبصمة المانيفست) لابقاء المسار الحتمي للتراجع.

عند الحاجة لعمليات purge، ولّد خطة قانونية بجانب الوصف:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

ارفق `portal.cache_plan.json` بحزمة DG-3 ليحصل المشغلون على مسارات/مضيفين حتميين (ومرشحات auth) عند ارسال طلبات `PURGE`. يمكن للقسم الاختياري في الوصف ان يشير الى هذا الملف مباشرة.

تحتاج حزمة DG-3 ايضا الى قائمة فحص للترقية والتراجع. ولّدها عبر `cargo xtask soradns-route-plan`:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

يسجل `gateway.route_plan.json` المضيفين القانونيين/الجميلين، تذكيرات فحوص الصحة المرحلية، تحديثات الربط GAR، purge للكاش، وخطوات التراجع. ارفقه مع ملفات GAR/binding/cutover قبل ارسال التذكرة حتى يتمكن فريق Ops من التدريب على الخطوات نفسها.

يقوم `scripts/generate-dns-cutover-plan.mjs` بإنشاء الوصف تلقائيا من `sorafs-pin-release.sh`. للتوليد اليدوي:

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

املأ البيانات الاختيارية عبر متغيرات البيئة قبل تشغيل المساعد:

| المتغير | الغرض |
| --- | --- |
| `DNS_CHANGE_TICKET` | معرف التذكرة المخزن في الوصف. |
| `DNS_CUTOVER_WINDOW` | نافذة cutover بصيغة ISO8601 (مثل `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | مضيف الانتاج + المنطقة المعتمدة. |
| `DNS_OPS_CONTACT` | جهة الاتصال للطوارئ. |
| `DNS_CACHE_PURGE_ENDPOINT` | نقطة purge المخزنة في الوصف. |
| `DNS_CACHE_PURGE_AUTH_ENV` | متغير البيئة الذي يحمل token purge (الافتراضي `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | مسار الوصف السابق لبيانات التراجع. |

ارفق JSON بمراجعة تغيير DNS حتى يتمكن المراجعون من التحقق من بصمة المانيفست وربط الاسماء المستعارة واوامر probe بدون فحص سجلات CI. توفر اعلام CLI مثل `--dns-change-ticket` و`--dns-cutover-window` و`--dns-hostname` و`--dns-zone` و`--ops-contact` و`--cache-purge-endpoint` و`--cache-purge-auth-env` و`--previous-dns-plan` نفس امكانية التخصيص خارج CI.

## الخطوة 8 - اصدار هيكل zonefile للمحلل (اختياري)

عند تحديد نافذة القطع للانتاج، يمكن لسكريبت الاصدار ان يصدر هيكل zonefile الخاص بـ SNS و snippet للمحلل تلقائيا. مرر سجلات DNS والبيانات عبر متغيرات البيئة او خيارات CLI؛ سيستدعي المساعد `scripts/sns_zonefile_skeleton.py` مباشرة بعد توليد الوصف. وفر على الاقل قيمة A/AAAA/CNAME وبصمة GAR (BLAKE3-256 للحمولة الموقعة). اذا كانت المنطقة/المضيف معروفة وتم حذف `--dns-zonefile-out`، سيكتب المساعد في `artifacts/sns/zonefiles/<zone>/<hostname>.json` ويولد `ops/soradns/static_zones.<hostname>.json` كـ resolver snippet.

| المتغير / الخيار | الغرض |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | مسار zonefile skeleton الناتج. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | مسار resolver snippet (الافتراضي `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL للسجلات الناتجة (الافتراضي: 600 ثانية). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | عناوين IPv4 (قائمة مفصولة بفواصل او علم متكرر). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | عناوين IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | هدف CNAME اختياري. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | بصمات SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | سجلات TXT اضافية (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | تجاوز label النسخة المحسوب. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | فرض timestamp `effective_at` (RFC3339) بدل بداية نافذة القطع. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | تجاوز proof المسجل في البيانات. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | تجاوز CID المسجل في البيانات. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | حالة تجميد الحارس (soft, hard, thawing, monitoring, emergency). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | مرجع تذكرة التجميد. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | timestamp RFC3339 لمرحلة thawing. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | ملاحظات تجميد اضافية (قائمة مفصولة بفواصل). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | بصمة BLAKE3-256 (hex) للحمولة الموقعة. مطلوبة عند وجود bindings للبوابة. |

تقرأ GitHub Actions هذه القيم من اسرار المستودع حتى ينتج كل pin للانتاج artefacts تلقائيا. اضبط الاسرار التالية (يمكن ان تحتوي القيم على قوائم مفصولة بفواصل):

| Secret | الغرض |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | مضيف/منطقة الانتاج للمساعد. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | جهة الاتصال المخزنة في الوصف. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | سجلات IPv4/IPv6 للنشر. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | هدف CNAME اختياري. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | بصمات SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | سجلات TXT اضافية. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | بيانات التجميد في الهيكل. |
| `DOCS_SORAFS_GAR_DIGEST` | بصمة BLAKE3 بالصيغة السداسية للحمولة الموقعة. |

عند تشغيل `.github/workflows/docs-portal-sorafs-pin.yml`، وفر المدخلات `dns_change_ticket` و`dns_cutover_window` حتى يرث الوصف/zonefile نافذة القطع الصحيحة. اتركها فارغة فقط لعمليات dry run.

مثال نموذجي (حسب runbook لـ SN-7 owner):

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

يسحب المساعد تذكرة التغيير تلقائيا كمدخل TXT ويورث بداية نافذة القطع الى `effective_at` ما لم يتم تجاوزها. للمسار التشغيلي الكامل راجع `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### قالب رؤوس البوابة

ينتج مساعد النشر ايضا `portal.gateway.headers.txt` و`portal.gateway.binding.json` وهما artefacts تلبي متطلبات DG-3 لربط محتوى البوابة:

- يحتوي `portal.gateway.headers.txt` على كتلة رؤوس HTTP كاملة (بما في ذلك `Sora-Name` و`Sora-Content-CID` و`Sora-Proof` وCSP وHSTS و`Sora-Route-Binding`) والتي يجب على بوابات الحافة لصقها في كل رد.
- يسجل `portal.gateway.binding.json` المعلومات نفسها بشكل مقروء آليا حتى تتمكن تذاكر التغيير والاتمتة من مقارنة host/cid دون كشط مخرجات shell.

يتم توليدها تلقائيا عبر `cargo xtask soradns-binding-template` وتلتقط alias وبصمة المانيفست وhostname الخاص بالبوابة الممرر الى `sorafs-pin-release.sh`. لاعادة التوليد او التخصيص:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

مرر `--csp-template` او `--permissions-template` او `--hsts-template` لتجاوز قوالب الرؤوس الافتراضية عند الحاجة، وادمجها مع مفاتيح `--no-*` لازالة رأس بالكامل.

ارفق مقطع الرؤوس بطلب تغيير CDN وادفع وثيقة JSON الى خط الالة الخاص بالبوابة حتى تتطابق ترقية المضيف مع ادلة الاصدار.

يشغّل سكربت الاصدار مساعد التحقق تلقائيا حتى تحتوي تذاكر DG-3 على ادلة حديثة. اعد تشغيله يدويا عند تعديل JSON الربط:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

تفك هذه الاوامر حمولة `Sora-Proof` المدبسة وتتاكد من ان بيانات `Sora-Route-Binding` تطابق CID المانيفست والـ hostname وتفشل اذا انحرف اي رأس. احتفظ بمخرجات الكونسول مع باقي artefacts عند تشغيل الامر خارج CI.

> **تكامل واصف DNS:** يدمج `portal.dns-cutover.json` الان قسما `gateway_binding` يشير الى هذه الملفات (المسارات وcontent CID وحالة proof والقالب النصي للرؤوس) **وكذلك** مقطع `route_plan` الذي يشير الى `gateway.route_plan.json` وقوالب الرؤوس الرئيسية/التراجع. ادرج هذه الاقسام في كل تذكرة DG-3 ليتمكن المراجعون من مقارنة قيم `Sora-Name/Sora-Proof/CSP` والتاكد من ان خطط الترقية/التراجع تطابق حزمة الادلة دون فتح الارشيف.

## الخطوة 9 - تشغيل مراقبات النشر

يتطلب بند خارطة الطريق **DOCS-3c** ادلة مستمرة على ان البوابة ووكيل Try it وروابط البوابة تظل سليمة بعد النشر. شغّل المراقب الموحد مباشرة بعد الخطوتين 7-8 واربطه بمسباراتك المجدولة:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- يقوم `scripts/monitor-publishing.mjs` بتحميل ملف الاعداد (راجع `docs/portal/docs/devportal/publishing-monitoring.md` للمخطط) وتنفيذ ثلاث فحوصات: مسارات البوابة مع تحقق CSP/Permissions-Policy، مسبارات وكيل Try it (اختياريا `/metrics`)، ومحقق ربط البوابة (`cargo xtask soradns-verify-binding`) الذي يفرض وجود قيمة Sora-Content-CID المتوقعة مع فحص alias/manifest.
- ينهي الامر بخروج غير صفري عند فشل اي probe حتى تتمكن CI/cron/فرق التشغيل من ايقاف الترقية.
- تمرير `--json-out` يكتب ملخص JSON واحد للحالات؛ و`--evidence-dir` يصدر `summary.json` و`portal.json` و`tryit.json` و`binding.json` و`checksums.sha256` حتى يتمكن مراجعو الحوكمة من مقارنة النتائج دون اعادة التشغيل. احفظ هذا الدليل تحت `artifacts/sorafs/<tag>/monitoring/` مع Sigstore bundle و DNS cutover descriptor.
- ارفق مخرجات المراقبة وتصدير Grafana (`dashboards/grafana/docs_portal.json`) ومعرف Alertmanager drill بتذكرة الاصدار ليكون SLO الخاص بـ DOCS-3c قابلا للتدقيق لاحقا. كتيب المراقبة مذكور في `docs/portal/docs/devportal/publishing-monitoring.md`.

تتطلب مسبارات البوابة HTTPS وترفض عناوين `http://` ما لم يتم تفعيل `allowInsecureHttp` في الاعداد. حافظ على TLS في بيئات الانتاج/التجربة وفعّل الاستثناء فقط للمعاينات المحلية.

يمكنك اتمتة المراقب عبر `npm run monitor:publishing` في Buildkite/cron بمجرد توفر البوابة. نفس الامر مع روابط الانتاج يغذي فحوصات الصحة المستمرة بين الاصدارات.

## الاتمتة عبر `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` يجمع الخطوات 2-6. فهو:

1. يؤرشف `build/` في tarball حتمي،
2. ينفذ `car pack` و`manifest build` و`manifest sign` و`manifest verify-signature` و`proof verify`،
3. ينفذ `manifest submit` اختياريا (بما فيه alias binding) عندما تتوفر بيانات اعتماد Torii،
4. يكتب `artifacts/sorafs/portal.pin.report.json` والاختياري `portal.pin.proposal.json` و DNS cutover descriptor وحزمة ربط البوابة (`portal.gateway.binding.json` مع كتلة الرؤوس) حتى تتمكن فرق الحوكمة والشبكات والعمليات من مراجعة الادلة دون قراءة سجلات CI.

اضبط `PIN_ALIAS` و`PIN_ALIAS_NAMESPACE` و`PIN_ALIAS_NAME` و(اختياريا) `PIN_ALIAS_PROOF_PATH` قبل تشغيل السكربت. استخدم `--skip-submit` للتجارب الجافة؛ يقوم workflow في GitHub بتبديل ذلك عبر `perform_submit`.

## الخطوة 8 - نشر مواصفات OpenAPI وحزم SBOM

يتطلب DOCS-7 ان يمر بناء البوابة ومواصفات OpenAPI و SBOM بنفس المسار الحتمي. تغطي الادوات الحالية الثلاثة:

1. **اعادة توليد وتوقيع المواصفات.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   استخدم `--version=<label>` لحفظ لقطة تاريخية (مثل `2025-q3`). يقوم المساعد بكتابة اللقطة الى `static/openapi/versions/<label>/torii.json` ويعكسها في `versions/current` ويحدث `static/openapi/versions.json` ببيانات SHA-256 وحالة المانيفست وتاريخ التحديث. يقرأ موقع المطورين هذا المؤشر ليعرض اختيار النسخة ومعلومات البصمة/التوقيع. اذا تم حذف `--version` فسيبقي التسميات السابقة ويحدث `current` و`latest` فقط.

   يلتقط المانيفست بصمات SHA-256/BLAKE3 حتى تتمكن البوابة من تدبيس `Sora-Proof` لـ `/reference/torii-swagger`.

2. **اصدار CycloneDX SBOMs.** يتوقع مسار الاصدار SBOMs عبر syft حسب `docs/source/sorafs_release_pipeline_plan.md`. احتفظ بالمخرجات قرب artefacts البناء:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **تغليف كل payload داخل CAR.**

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

   اتبع نفس خطوات `manifest build` / `manifest sign` الخاصة بالموقع الرئيسي، واضبط aliases لكل artefact (مثل `docs-openapi.sora` للمواصفات و`docs-sbom.sora` لحزمة SBOM الموقعة). يبقي ذلك SoraDNS و GAR وتذاكر التراجع مقيدة بكل payload.

4. **الارسال والربط.** اعادة استخدام authority والـ Sigstore bundle، مع تسجيل alias tuple في قائمة فحص الاصدار حتى يعرف المدققون اي اسم Sora يقابل اي بصمة.

يساعد حفظ مانيifestات OpenAPI/SBOM مع بناء البوابة على ان يحتوي كل ticket على مجموعة الادلة الكاملة دون اعادة تشغيل packer.

### مساعد الاتمتة (CI/package script)

`./ci/package_docs_portal_sorafs.sh` يجمع الخطوات 1-8 في امر واحد. يقوم المساعد بـ:

- اعداد البوابة (`npm ci`، مزامنة OpenAPI/Norito، اختبارات widgets).
- اصدار CARs ومانيفستات للبوابة و OpenAPI و SBOM عبر `sorafs_cli`.
- تنفيذ `sorafs_cli proof verify` (`--proof`) والتوقيع عبر Sigstore (`--sign`، `--sigstore-provider`، `--sigstore-audience`) اختياريا.
- وضع كل الاصول تحت `artifacts/devportal/sorafs/<timestamp>/` وكتابة `package_summary.json` لادوات CI/release.
- تحديث `artifacts/devportal/sorafs/latest` ليشير الى اخر تشغيل.

مثال (خط كامل مع Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

اهم الاعلام:

- `--out <dir>` - تغيير جذر الاصول (الافتراضي مجلدات timestamp).
- `--skip-build` - اعادة استخدام `docs/portal/build` الموجود.
- `--skip-sync-openapi` - تجاوز `npm run sync-openapi` عندما لا يستطيع `cargo xtask openapi` الوصول الى crates.io.
- `--skip-sbom` - عدم استدعاء `syft` عند عدم توفره (مع تحذير).
- `--proof` - تنفيذ `sorafs_cli proof verify` لكل زوج CAR/manifest.
- `--sign` - تنفيذ `sorafs_cli manifest sign`.

للانتاج استخدم `docs/portal/scripts/sorafs-pin-release.sh`. يجمع portal/OpenAPI/SBOM ويوقّع المانيفستات ويسجل metadata اضافية في `portal.additional_assets.json`. يدعم مفاتيح `--openapi-*` و`--portal-sbom-*` و`--openapi-sbom-*` لتعيين alias لكل artefact وتجاوز مصدر SBOM عبر `--openapi-sbom-source` وتخطي بعض payloads (`--skip-openapi`/`--skip-sbom`) وتحديد `syft` غير الافتراضي عبر `--syft-bin`.

يعرض السكربت كل الاوامر؛ انسخ السجل الى تذكرة الاصدار مع `package_summary.json` حتى يتمكن المراجعون من مقارنة digests وmetadata دون تتبع خرج shell يدوي.

## الخطوة 9 - التحقق من gateway + SoraDNS

قبل اعلان cutover، اثبت ان alias الجديد يحل عبر SoraDNS وان البوابات تدبس proofs جديدة:

1. **تشغيل probe gate.** يقوم `ci/check_sorafs_gateway_probe.sh` بتشغيل `cargo xtask sorafs-gateway-probe` على fixtures في `fixtures/sorafs_gateway/probe_demo/`. للتشغيل الفعلي، وجّه الاختبار الى الـ hostname المطلوب:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   يفك probe رؤوس `Sora-Name` و`Sora-Proof` و`Sora-Proof-Status` وفق `docs/source/sorafs_alias_policy.md` ويفشل عند انحراف بصمة المانيفست او TTLs او GAR bindings.

   For lightweight spot checks (for example, when only the binding bundle
   changed), run `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   The helper validates the captured binding bundle and is handy for release
   tickets that only need binding confirmation instead of a full probe drill.

2. **جمع ادلة drills.** استخدم `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...` لتجميع الرؤوس والسجلات في `artifacts/sorafs_gateway_probe/<stamp>/` وتحديث `ops/drill-log.md`، مع امكانية تشغيل rollback hooks او PagerDuty. اضبط `--host docs.sora` للتحقق من مسار SoraDNS.

3. **التحقق من DNS bindings.** عند نشر proof الخاص بالalias، احفظ ملف GAR المشار اليه وارفقة مع ادلة الاصدار. يمكن لمشغلي resolver اعادة الاختبار عبر `tools/soradns-resolver`. قبل ارفاق JSON، نفّذ `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]` للتحقق من host mapping وmetadata والـ telemetry labels. يمكن للمساعد اصدار ملخص `--json-out` بجانب GAR الموقّع.
  عند صياغة GAR جديد، فضّل `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`، ولا تستخدم `--manifest-cid` الا عند غياب ملف المانيفست. يقوم المساعد الان باشتقاق CID وبصمة BLAKE3 مباشرة من JSON ويزيل المسافات ويزيل تكرارات labels ويرتبها ويضيف قوالب CSP/HSTS/Permissions-Policy قبل الكتابة لضمان الحتمية.

4. **مراقبة مقاييس alias.** ابق `torii_sorafs_alias_cache_refresh_duration_ms` و`torii_sorafs_gateway_refusals_total{profile="docs"}` على الشاشة؛ كلاهما معروض في `dashboards/grafana/docs_portal.json`.

## الخطوة 10 - المراقبة وحزم الادلة

- **اللوحات.** صدّر `dashboards/grafana/docs_portal.json` و`dashboards/grafana/sorafs_gateway_observability.json` و`dashboards/grafana/sorafs_fetch_observability.json` لكل اصدار وارفقة مع التذكرة.
- **ارشفة probes.** احتفظ بـ `artifacts/sorafs_gateway_probe/<stamp>/` ضمن git-annex او مخزن الادلة.
- **حزمة الاصدار.** خزّن ملخصات CAR ومجموعات المانيفست وتواقيع Sigstore و`portal.pin.report.json` وسجلات Try-It وتقارير الروابط تحت مجلد واحد بتاريخ.
- **سجل drills.** عند تشغيل drills، يحدث `scripts/telemetry/run_sorafs_gateway_probe.sh` ملف `ops/drill-log.md` لتلبية متطلبات SNNet-5.
- **روابط التذاكر.** اربط معرفات لوحات Grafana او صادرات PNG مع مسار تقرير probe حتى يتمكن المراجعون من التحقق دون وصول shell.

## الخطوة 11 - تمرين fetch متعدد المصادر وادلة scoreboard

اصبح نشر SoraFS يتطلب ادلة fetch متعددة المصادر (DOCS-7/SF-6) مع ادلة DNS/gateway. بعد تثبيت المانيفست:

1. **تشغيل `sorafs_fetch` على المانيفست الحي.** استخدم نفس plan/manifest مع بيانات اعتماد البوابة لكل مزود واحفظ جميع المخرجات:

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

   - احصل اولا على provider adverts المشار اليها في المانيفست (مثل `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) ومررها عبر `--provider-advert name=path` لضمان تقييم القدرات بشكل حتمي. استخدم `--allow-implicit-provider-metadata` **فقط** عند اعادة تشغيل fixtures في CI.
   - عند وجود مناطق اضافية، اعد التشغيل مع مزوديها للحصول على ادلة لكل cache/alias.

2. **ارشفة المخرجات.** خزّن `scoreboard.json` و`providers.ndjson` و`fetch.json` و`chunk_receipts.ndjson` في دليل الادلة.

3. **تحديث القياسات.** استورد المخرجات في لوحة **SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`) وراقب `torii_sorafs_fetch_duration_ms`/`_failures_total`.

4. **فحص قواعد التنبيه.** شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` وارفق مخرجات promtool.

5. **الدمج في CI.** فعّل خطوة `sorafs_fetch` عبر `perform_fetch_probe` في staging/production لتوليد الادلة تلقائيا.

## الترقية والملاحظة والتراجع

1. **الترقية:** احتفظ بـ aliases منفصلة لـ staging وproduction. قم بالترقية عبر اعادة `manifest submit` بالمانيفست نفسه مع تبديل `--alias-namespace/--alias-name` الى alias الانتاج.
2. **المراقبة:** استخدم `docs/source/grafana_sorafs_pin_registry.json` ومسبارات البوابة الخاصة في `docs/portal/docs/devportal/observability.md`.
3. **التراجع:** لاعادة التراجع، اعد ارسال المانيفست السابق (او اسحب alias الحالي) عبر `sorafs_cli manifest submit --alias ... --retire`.

## قالب CI

يجب ان يتضمن الحد الادنى من خط الانابيب:

1. Build + lint (`npm ci`, `npm run build`, توليد checksums).
2. التغليف (`car pack`) وحساب المانيفست.
3. التوقيع باستخدام OIDC token الخاص بالوظيفة (`manifest sign`).
4. رفع الاصول (CAR، مانيفست، bundle، plan، summaries).
5. الارسال الى pin registry:
   - Pull requests -> `docs-preview.sora`.
   - Tags / فروع محمية -> ترقية alias الانتاج.
6. تشغيل probes + بوابات التحقق من proof قبل الانتهاء.

`.github/workflows/docs-portal-sorafs-pin.yml` يربط هذه الخطوات لاصدارات يدوية. يقوم الـ workflow بـ:

- بناء/اختبار البوابة،
- تغليف build عبر `scripts/sorafs-pin-release.sh`،
- توقيع/تحقق manifest bundle عبر GitHub OIDC،
- رفع CAR/manifest/bundle/plan/proof summaries كـ artifacts،
- (اختياريا) ارسال المانيفست وربط alias عند توفر secrets.

اضبط secrets/variables التالية قبل التشغيل:

| Name | الغرض |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | مضيف Torii الذي يكشف `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | معرف epoch المسجل مع الارسال. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | سلطة التوقيع لارسال المانيفست. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | alias tuple المربوط عند `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | bundle proof للalias بترميز base64 (اختياري). |
| `DOCS_ANALYTICS_*` | نقاط analytics/probe الحالية. |

شغّل الـ workflow عبر واجهة Actions:

1. زود `alias_label` (مثل `docs.sora.link`)، و`proposal_alias` اختياري، و`release_tag` اختياري.
2. اترك `perform_submit` غير مفعل لتوليد الاصول دون لمس Torii (dry run) او فعّله للنشر المباشر على alias.

يبقى `docs/source/sorafs_ci_templates.md` كمرجع لقوالب عامة، لكن workflow الخاص بالبوابة هو المسار المفضل للاصدارات اليومية.

## قائمة التحقق

- [ ] نجاح `npm run build` و`npm run test:*` و`npm run check:links`.
- [ ] حفظ `build/checksums.sha256` و`build/release.json` ضمن الاصول.
- [ ] توليد CAR وplan وmanifest وsummary تحت `artifacts/`.
- [ ] حفظ Sigstore bundle والتوقيع المنفصل مع السجلات.
- [ ] حفظ `portal.manifest.submit.summary.json` و`portal.manifest.submit.response.json` عند الارسال.
- [ ] ارشفة `portal.pin.report.json` (و`portal.pin.proposal.json` عند توفرها).
- [ ] حفظ سجلات `proof verify` و`manifest verify-signature`.
- [ ] تحديث لوحات Grafana ونجاح probes Try-It.
- [ ] ارفاق ملاحظات التراجع (ID المانيفست السابق + digest alias) مع تذكرة الاصدار.
