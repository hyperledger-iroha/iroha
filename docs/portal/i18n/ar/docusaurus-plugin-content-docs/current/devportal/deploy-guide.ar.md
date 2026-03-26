---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## نظرة عامة

تحويل هذا الدليل الاجرائي عناصر خريطة الطريق **DOCS-7** (نشر SoraFS) و **DOCS-8** (اتمتة دبوس في CI/CD) إلى الإجراءات العملية لبوابة المطورين. يغطي مرحلة البناء/الوبر، أفلام SoraFS، توقيع المانيفست عبر Sigstore، ترقية الاسماء المستعارة، التحقق، وتمارين الحكمة بحيث تكون كل مشاهدة واصدار قابل للطباعة لاعادة الانتاج وقابلة للدقيق.

يشير هذا المسار الى ان لديك ثنائي `sorafs_cli` (مبني عبر `--features cli`)، وصولا الى Torii endpoint مع صلاحيات pin-registry، وبيانات تعتمد OIDC لـ Sigstore. خزين الاسرار طويل العمر (`IROHA_PRIVATE_KEY`، `SIGSTORE_ID_TOKEN`، رموز Torii) في خزينة CI؛ ويمكن للعمليات المحلية تحميلها عبر exports من الـ shell.

##المتطلبات المسبقة

- Node 18.18 او احدث مع `npm` او `pnpm`.
- `sorafs_cli` من `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- عنوان Torii يكشف `/v1/sorafs/*` مع حساب/مفتاح خاص بسلطة سلطة سلطة المانيفست والاسماء المستعارة.
- مُصدِر OIDC (GitHub Actions، GitLab، Workload Identity، الخ) لاإصدار `SIGSTORE_ID_TOKEN`.
- اختياري: `examples/sorafs_cli_quickstart.sh` للتجارب الجافة و `docs/source/sorafs_ci_templates.md` لقوالب سير العمل في GitHub/GitLab.
- اضبط تنوعات OAuth الخاصة بـ Try it (`DOCS_OAUTH_*`) وشغّل
  [قائمة مراجعة تشديد الأمان](./security-hardening.md) قبل إنشاء المختبر الخارجي. تفشل البوابة في فشل الان عند غياب هذه الكلمة او عند نهاية مفاتيح TTL/polling عن نوافذ جديدة؛ استخدم `DOCS_OAUTH_ALLOW_INSECURE=1` فقط لمعاينات مؤقتة. ارفق ادلة اختبار الاختراق مع تذكرة الاصدار.

## الخطوة 0 - التقاط حزمة وكيل Try it

قبل المعاينة الى Netlify البوابة او، ثبّت مصادر وكيل Try it وبصمة المانيفست OpenAPI الموقّع في الحزمة حتمية:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

يقوم `scripts/tryit-proxy-release.mjs` بنسخ ادوات الوكيل/المسبار/التراجع، والتحقق من توقيع OpenAPI وكتابة `release.json` مع `checksums.sha256`. ارفق هذه الحزمة مع تذكرة ترقية Netlify/SoraFS gate كي أسرع المراجعون من إعادة تشغيل المصادر المباشرة والتلميحات Torii بدون إعادة بناء. وسجلت أيضا ما اذا كان تم الحصول على الرموز المميزة لحاملها المرسلة من العميل (`allow_client_auth`) الامطار على اتساق خطة الاطلاق وقواعد CSP.

## الخطوة 1 - بناء وفحص الوبر للبوابة

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

ينفذ `npm run build` `scripts/write-checksums.mjs` وينتج:

- `build/checksums.sha256` - مانيفست SHA256 صالح لـ `sha256sum -c`.
- `build/release.json` - بيانات وصفية (`tag`، `generated_at`، `source`) مثبتة في كل CAR/MANIFST.

احفظ كلاين الملف مع ملخص CAR حتى تتنوع المراجعون من مقارنة نواتج المعاينة بدون إعادة بناء.

## الخطوة 2 - مكونات الاصول الثابتة

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

يلتقط ملخص JSON عدد الـ قطع، والـ ملخصات، وتلميحات تخطيط إثبات التي يعيد `manifest build` ولوحات CI يستخدم لاحقاً.

## الخطوة 2ب - مرافقات المرافقين OpenAPI و SBOM

يلزم DOCS-7 نشر موقع البوابة ولقطة OpenAPI و SBOM كمانيفستات فرض حظر على البوابات من تدابيس الكتابة `Sora-Proof`/`Sora-Content-CID` لكل artefact. يقوم مساعد الاصدار (`scripts/sorafs-pin-release.sh`) بالفعل بتغليف المجلد OpenAPI (`static/openapi/`) SBOM يبدأ عبر `syft` في CARs بروتين `openapi.*`/`*-sbom.*` ويسجل البيانات في `artifacts/sorafs/portal.additional_assets.json`. عند اتباع المسار اليدوي، أعد الخطوات 2-4 لكل حمولة مع مبادئها ووسوم البيانات الوصفية الخاصة بها (مثلا `--car-out "$OUT"/openapi.car` مع `--metadata alias_label=docs.sora.link/openapi`). سجل كل زوج البيان/الاسم المستعار في Torii (الموقع، OpenAPI، SBOM البوابة، SBOM OpenAPI) قبل تغيير DNS حتى البداية الجديدة من تقديم البروفات مدبسة لكل قطعة أثرية منشور.

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

تعدّل اعلام سياسة الدبوس بما في ذلك إمكانية الوصول مع نافذة الاصدار (مثلا `--pin-storage-class hot` للكناري). نسخة JSON اختيارية ولكن مفيدة للمراجعة.

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

سجل الـ الحزمة بصمة المانيفست وبصمات الـ Chunks وهاش BLAKE3 للـ OIDC رمز دون حفظ JWT. تستخدم بالـ الحزمة والتوقيع المنفصل؛ يمكن الترقية في إنتاج إعادة استخدام نفس الاصول بدون إعادة صياغة. يمكن للعمليات المحلية استبدال اعلام المورد بـ `--identity-token-env` (او تعيين `SIGSTORE_ID_TOKEN` في البيئة) عندما يصبح مساعد OIDC خارجي التوكن.

## الخطوة 5 - الارسال الى دبوس التسجيل

ارسل المانيفست الموقّع (وخطة الـ Chunks) الى Torii. اطلب دائما الملخص كي تبقى نتيجة التسجيل/الاسم المستعار قابل للدقيق.

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

عند معاينة او كاناري (`docs-preview.sora`)، قم بالارسال باستخدام اسم مستعار مختلف حتى تسعى لضمان الجودة من التحقق قبل ترقية الإنتاج.

يلزم الاسم المستعار ثلاثة مشتركين: `--alias-namespace` و `--alias-name` و `--alias-proof`. وينتج فريق الـحزمة إثبات (base64 او Norito bytes) عند الموافقة؛ خزنه ضمن اسرار CI ومديره كملف الاتصال قبل `manifest submit`. اترك الاسم المستعار فارغًا إذا كنت ترغب في تثبيت المانيفيست بدون تغيير DNS.

## الخطوة 5 ب - إنشاء و إنشاء

يجب ان يرافق كل مانيفيست الجاهزية للبرلمان بحيث يستطيع أي مواطن من سورا تقديم الجديد دون الحصول على بيانات مميزة. بعد خطوات submit/sign نفّذ:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

يلتقط `portal.pin.proposal.json` التعليمية القياسية `RegisterPinManifest` وبصمة الـ Chunks والسياسة وتلميح alias. ارفقه مع تذكرة الـ تور بوابة او التوليد كي يبدأ المندوبون من مراجعة الـ payload دون إعادة البناء. هذا الأمر لا يلمس مفتاح Torii، لذلك يمكن لاي مواطن ان ينشئ مقترحا محليا.

## الخطوة 6 – التصديق على البراهين والقياسات

بعد التثبيت، تنفيذ خطوات التحقق الحتمية:

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

- راقب `torii_sorafs_gateway_refusals_total` و `torii_sorafs_replication_sla_total{outcome="missed"}` لمعرفة الشذوذ.
- شغّل `npm run probe:portal` وكيل وكيل Try-It وروابط التحقق من الفيديو المثبت.
- جمع المعادلة المذكورة في
  [النشر والمتابعة](./publishing-monitoring.md) بوابة الفرص القوية DOCS-3c. يدعم المساعدة الان عدة `bindings` (الموقع، OpenAPI، SBOM للبوابة، SBOM لـ OpenAPI) ويفرض `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` على السيطرة على الكرة `hostname`. الاستدعاء ادناه يكتب ملخص JSON واحدا وزمة المعادلة (`portal.json`، `tryit.json`، `binding.json`، `checksums.sha256`) تحت مجلد الاصدار:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## الخطوة 6a - تخطيط شهادات البوابة

استخرج خطة TLS SAN/challenge قبل توليد حزم GAR حتى يراجع فريق البوابة وموافقو DNS نفس البديل. يعكس المساعد الجديد المدخلات DG-3 عبر تعداد رقميين Wildcard و SAN لـ beautiful-host وتسميات DNS-01 والتحديات لذلك:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

قم بضم JSON مع حزمة الاصدار (او ارفقه مع تذكرة التغيير) حتى يتمكن من البدء في لصق قيم SAN داخل إعدادات `torii.sorafs_gateway.acme` ويتأكد من زائرو GAR من قارئ canonical/pretty دون إعادة الحساب. اضف `--name` لكل لاحقة من الحرير يتم ترقيتها في الاصدار بنفسه.

## مهمة 6ب - اشتقاق خرائط أصلية قانونية

قبل إنشاء مولد GAR، سجّل الخريطة المحلية الحتمية لكل اسم مستعار. يقوم `cargo xtask soradns-hosts` بتهشئة كل `--name` الى التسمية القانونية (`<base32>.gw.sora.id`) ويصدر حرف البدل المطلوب (`*.gw.sora.id`) ويشتق الشرفة الجميلة (`<alias>.gw.sora.name`). احفظ النتيجة ضمن المصنوعات اليدوية الاصدار كي يختار المراجعين DG-3 من مقارنة الخريطة مع مكاتب GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

استخدم `--verify-host-patterns <file>` للفشل السريع عندما يغيب أحد المضيفين المطلوبين من GAR او JSON الخاص بالربط. يقبل عدة نماذج من الملفات، مما يشكل تكوين دقيق لقالب GAR و `portal.gateway.binding.json` في نفس الاستدعاء:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

ارفق ملخص JSON وتحقق التحقق من تذكرة تغيير DNS/البوابة حتى يتمكن المدققون المعتمدون بالتأكيد من صحة/الجميلة دون إعادة السكربتات. اعد تشغيل الأمر عند إضافة الاسم المستعار الجديد حتى ترث تحديثات GAR نفس المعادلة.

## الخطوة 7 - إنشاء واصف DNS Cover

تتطلب عمليات القطع في حزمة الإنتاج القابلة للتغيير الدقيق. بعد نجاح الارسال (ربط الاسم المستعار)، يصدر المساعد
`artifacts/sorafs/portal.dns-cutover.json`، ويتضمن:

- بيانات ربط الاسم المستعار (مساحة الاسم/الاسم/الإثبات، بصمة المانيفست، عنوان Torii، عصر الارسال، السلطة).
- مؤتمر الاصدار (tag، alias label، مسارات المانيفيست/CAR، خطة الـ Chunks، Sigstore package).
- مؤشر التحقق (امر التحقيق، الاسم المستعار + Torii نقطة النهاية).
- عدم تغيير الاختيارية (معرف التذكرة، نافذة القطع، جهة الاتصال التشغيلية، المضيف/منطقة الإنتاج).
- بيانات ترقية المسار المشتقة من رأس `Sora-Route-Binding` (المضيف الأخضر/CID، مسارات الرأس + الهبوط، اوامر التحقق) و ان ترقية GAR و تمارين مون تشير لنفس العادله.
- ملفات road-plan المولدة (`gateway.route_plan.json`، قوالب الرؤوس، ورؤوس المشاهير الاختيارية) حتى تتحقق تذاكر DG-3 و الوبر في CI من مؤتمر الترقية/التراجع.
- بيانات اختيارية للغاء الكاش (نقطة التطهير، مصادقة عتيقة، حمولة JSON، ومثال `curl`).
- تلميحات تشير إلى الوصف السابق (وسم الاصدار وبصمة المانيفست) لاستمرار البقاء حتمي للتراجع.

عند الحاجة إلى التطهير، ولضبط خطة تعتمد على الوصف:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

ارفق `portal.cache_plan.json` بحزمة DG-3 ليحصل على التشغيل على مسارات/مضيفين حطيميين (ومرشحين auth) عند إرسال طلبات `PURGE`. يمكن للقسم الاختياري في الوصف ان يشير الى هذا الملف مباشرة.

ستحتاج أيضًا إلى حزمة DG-3 للرجوع للفحص للترقية والتراجع. ولّدها عبر `cargo xtask soradns-route-plan`:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

سجل `gateway.route_plan.json` المسجلين/الجميلين، تذكيرات فحوصات الصحة المرحلية، تحديثات التراجع GAR، purge للكاش، وخطوات مارس. ارفقه مع ملفات GAR/binding/cutover قبل إرسال التذكرة حتى يريد فريق Ops من التدريب على الخطوات الذاتية.

هل `scripts/generate-dns-cutover-plan.mjs` غير موجود فعليًا من `sorafs-pin-release.sh`. للتوليد اليدوي:

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

املأ البيانات الاختيارية عبر المتغيرات البيئية قبل تشغيل المساعدة:| المتغير | اللحوم |
| --- | --- |
| `DNS_CHANGE_TICKET` | معرف التذكرة المخزن في الوصف. |
| `DNS_CUTOVER_WINDOW` | نافذة القطع بصيغة ISO8601 (مثل `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`، `DNS_ZONE` | استضافة الإنتاج + المنطقة المعتمدة. |
| `DNS_OPS_CONTACT` | جهة الاتصال للطوارئ. |
| `DNS_CACHE_PURGE_ENDPOINT` | مخزن نقطة التطهير في الوصف. |
| `DNS_CACHE_PURGE_AUTH_ENV` | نسق البيئة الذي يقوم بعصر الرمز المميز (الافتراضي `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | المسار السابق الوصف لبيانات السماء. |

ارفق JSON بمراجعة فحص تغيير DNS حتى تعدى المراجعون من التحقق من البصمة المانيفست وربط الاسماء المستعارة واوامر مسبار بدون سجلات CI. قوة اعلام CLI مثل `--dns-change-ticket` و`--dns-cutover-window` و`--dns-hostname` و`--dns-zone` و`--ops-contact` و`--cache-purge-endpoint` و`--cache-purge-auth-env` و`--previous-dns-plan` *نفس التخصيص خارج CI.

## الخطوة 8 - اصدار هيكل Zonefile للمحل (اختياري)

عند تحديد نافذة القطع للانتاج، يمكن إصدار السكربت ان يصدر هيكل Zonefile الخاص بـ SNS و snippet للمحلل مباشرة. مرر أرشيفات DNS والبيانات عبر متغيرات البيئة أو خيارات CLI؛ سيستدعي المساعد `scripts/sns_zonefile_skeleton.py` مباشرة بعد توليد الوصفة. وفر على القيمة A/AAAA/CNAME وبصمة GAR (BLAKE3-256 للحمولة الموقعة). اذا كانت المنطقة/المضيف معروف وتم حذف `--dns-zonefile-out`، سيكتب المساعد في `artifacts/sns/zonefiles/<zone>/<hostname>.json` ويولد `ops/soradns/static_zones.<hostname>.json` كـ Resolver snippet.

| المتغير / خيار | اللحوم |
| --- | --- |
| `DNS_ZONEFILE_OUT`، `--dns-zonefile-out` | مسار Zonefile الهيكل العظمي الناتج. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`، `--dns-zonefile-resolver-snippet` | مقتطف محلل المسار (الافتراضي `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`، `--dns-zonefile-ttl` | TTL للسجلات الناشئة (الافتراضي: 600 ثانية). |
| `DNS_ZONEFILE_IPV4`، `--dns-zonefile-ipv4` | عناوين IPv4 (قائمة مفصولة بفواصل او علم). |
| `DNS_ZONEFILE_IPV6`، `--dns-zonefile-ipv6` | عناوين IPv6. |
| `DNS_ZONEFILE_CNAME`، `--dns-zonefile-cname` | هدف CNAME اختياري. |
| `DNS_ZONEFILE_SPKI`، `--dns-zonefile-spki-pin` | بصمات SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`، `--dns-zonefile-txt` | سجلات TXT محمية (`key=value`). |
| `DNS_ZONEFILE_VERSION`، `--dns-zonefile-version` | تجاوز التسمية النسخة المحفوظة. |
| `DNS_ZONEFILE_EFFECTIVE_AT`، `--dns-zonefile-effective-at` | يفترض الطابع الزمني `effective_at` (RFC3339) الإصدار الجديد من ويندوز. |
| `DNS_ZONEFILE_PROOF`، `--dns-zonefile-proof` | تجاوز إثبات المسجل في البيانات. |
| `DNS_ZONEFILE_CID`، `--dns-zonefile-cid` | تجاوز CID المسجل في البيانات. |
| `DNS_ZONEFILE_FREEZE_STATE`، `--dns-zonefile-freeze-state` | حالة تجميد الجارديان (لينة، صلبة، ذوبان، مراقبة، طوارئ). |
| `DNS_ZONEFILE_FREEZE_TICKET`، `--dns-zonefile-freeze-ticket` | مرجع تذكرة التجميد. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`، `--dns-zonefile-freeze-expires-at` | الطابع الزمني RFC3339 لمرحلة الذوبان. |
| `DNS_ZONEFILE_FREEZE_NOTES`، `--dns-zonefile-freeze-note` | قائمة تجميد فريزر (قائمة مفصولة بفواصل). |
| `DNS_GAR_DIGEST`، `--dns-gar-digest` | بصمة BLAKE3-256 (ست عشري) للموقع المحمول. مطلوبة عند وجود روابط للبوابة. |

تقرأ GitHub Actions هذه القيم من اسرار المستودع حتى ينتج كل دبوس للانتاج الأثري باللون الأسود. اضبط الاسرار التالية (يمكن ان تحتوي على القيم في القوائم مفصولة بفواصل):

| سر | اللحوم |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`، `DOCS_SORAFS_DNS_ZONE` | المضيف/منطقة الإنتاج للمساعد. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | جهة الاتصال مخزنة في الوصف. |
| `DOCS_SORAFS_ZONEFILE_IPV4`، `DOCS_SORAFS_ZONEFILE_IPV6` | سجلات IPv4/IPv6 للنشر. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | هدف CNAME اختياري. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | بصمات SPKI قاعدة64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | سجلات TXT. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | بيانات التجميد في الهيكل. |
| `DOCS_SORAFS_GAR_DIGEST` | بصمة BLAKE3 بصيغة السداسية للموقع المحمول. |

عند تشغيل `.github/workflows/docs-portal-sorafs-pin.yml`، وفر المدخلات `dns_change_ticket` و`dns_cutover_window` حتى يرث الوصف/zonefile نافذة الاكتشاف الصحيح. اتركها فارغة فقط للتشغيل الجاف.

مثال نموذجي (حسب دليل التشغيل لـ مالك SN-7):

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

يسحب المساعدة تذكرة الغد القادم كمدخل TXT ويورث بداية نافذة القطع الى `effective_at` ما لم يتم تجاوزها. للمسار التشغيلي الكامل رجوع `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### ملاحظة حول إحالة DNS العام

هيكل ملف المنطقة يتم تعريفه فقط على سجلات المنطقة الموثوقة. ما نحتاجه لضبط مرشح NS/DS للوصول للمدير او مستشار DNS حتى يكتشف الإنترنت العام من العثور على الاسماء.

- وقم بالتقطيع عند apex/TLD استخدم ALIAS/ANAME (حسب المزود) او نشر الارشيف A/AAAA شاهد الى عناوين Anycast الخاصة بالبوابة.
- للنطاقات النظرية، انشر CNAME الى الـ Pretty host المشتق (`<fqdn>.gw.sora.name`).
- المسجل (`<hash>.gw.sora.id`) يبقى تحت نطاق البوابة ولا يُنشر نطاقك العام.

### قالب الحروف البوابة

ونتيجة لذلك، `portal.gateway.headers.txt` و`portal.gateway.binding.json` وartefacts متطلبات DG-3 لربط محتوى البوابة:

- يحتوي على `portal.gateway.headers.txt` على كتلة العناوين HTTP كاملة (بما في ذلك `Sora-Name` و`Sora-Content-CID` و`Sora-Proof` وCSP وHSTS و`Sora-Route-Binding`) والتي يجب على بوابات هيكل لصقها في كل رد.
- تم تسجيل `portal.gateway.binding.json` المعلومات الخاصة به بشكل مقروء آليا حتى بدء تسجيل التغيير والمتعة من مقارنة المضيف/cid دون كشطات shell.

يتم توليدها مباشرة عبر `cargo xtask soradns-binding-template` واختيار الاسم المستعار وبصمة المانيفست واسم المضيف الخاص بالبوابة الممرر الى `sorafs-pin-release.sh`. لاعادة التوليد او التخصيص:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

مرر `--csp-template` او `--permissions-template` او `--hsts-template` لتجاوز قوالب الرؤوس الافتراضية عند الحاجة، ودمجها مع مفاتيح `--no-*` لازالة رأس بالكامل.

ارفق جزء من الرأس الفردي بـ CDN ودفع وثيقة JSON الى خط الاالة الخاص بالبوابة حتى تتطابق المنافسة مع منافسة الاصدار.

يحتوي على سكربت مساعد الاصدار المعتمد حتى التذاكر DG-3 على ما يعادلها حديثا. اعد تشغيلهاليا عند تعديل JSON الجنوبي:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

تفك هذه الاوامر المحمولة `Sora-Proof` المدبسة وتأكد من ان بيانات `Sora-Route-Binding` تطابق CID المانيفست والـ hostname وفشل اذا انحرف اي رأس. احتفظ بمخرجات الكونسول مع باقي القطع الأثرية عند الأمر خارج CI.

> **تكامل واصف DNS:** يدمج `portal.dns-cutover.json` الان قسما `gateway_binding` يشير الى هذه الملفات (المسارات وcontent CID وحالة إثبات والقالب النصي للرؤوس) **وك ذلك** المقطع `route_plan` الذي يشير الى `gateway.route_plan.json` وقوالب الرؤوس الرئيسية/التراجع. ادرج هذه الاقسام في كل تذكرة DG-3 ليتمكن من الوصول إلى المراجع من مقارنة القيم `Sora-Name/Sora-Proof/CSP` والتأكد من انجاز الترقية/التراجع تطابق حزمة المعادلة دون فتح الارشيف.

## الخطوة 9 - مراقبات النشر

تتطلب بند خريطة الطريق **DOCS-3c** تكامل مستمر على ان البوابة والوكيل جربها وروابط الشعار للتحكم بشكل سليم بعد النشر. شغّل المراقب الموحد مباشرة بعد الخطوتين 7-8 واربطه بمسبارك المجدولة:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- يقوم `scripts/monitor-publishing.mjs` بتحميل ملف الاعداد (راجع `docs/portal/docs/devportal/publishing-monitoring.md` للمخطط) شباب فحوصات: مسارات البوابة مع تحقق CSP/Permissions-Policy، مسبارات وكيل Try it (اختياريا `/metrics`)، ومحقق ربط البوابة (`cargo xtask soradns-verify-binding`) الذي يفرض وجود Sora-Content-CID مع فحص الاسم المستعار/البيان.
- ينهي الأمر بخروج غير صفري عند فشل اي مسبار حتى CI/cron/فرق التشغيل من ايقاف الترقية.
- تسارع `--json-out` وكاتب ملخص JSON واحد للحالات؛ و`--evidence-dir` يتقدم `summary.json` و`portal.json` و`tryit.json` و`binding.json` و`checksums.sha256` حتى يعودو التعدد من مقارنة النتائج دون إعادة التشغيل. احفظ هذا الدليل تحت `artifacts/sorafs/<tag>/monitoring/` مع حزمة Sigstore واصف تحويل DNS.
- ارفق مخرجات التحرير وتصدير Grafana (`dashboards/grafana/docs_portal.json`) ومعرف Alertmanager Drill بتذكرة الاصدار ليكون SLO الخاص بـ DOCS-3c قابلا للدقيق لاحقاً. ضابط الدفاع في `docs/portal/docs/devportal/publishing-monitoring.md`.

تتطلب مسبارات البوابة HTTPS وترفض العناوين `http://` ما لم يتم تفعيل `allowInsecureHttp` في الاعداد. حافظ على TLS في بيئات الإنتاج/التجربة وفعّل الاستثناء فقط للمعاينات المحلية.

يمكنك الاستمتاع بالمراقبة عبر `npm run monitor:publishing` في Buildkite/cron بمجرد توفر البوابة. نفس الأمر مع روابط الإنتاج الغذائي فحوصات الصحة بين الاصدارات.

##المتعة عبر `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` متابعة الخطوات 2-6. هو:

1. يؤرشف `build/` في القطران حتمي،
2. ينفذ `car pack` و`manifest build` و`manifest sign` و`manifest verify-signature` و`proof verify`،
3. ينفذ `manifest submit` اختياريا (بما فيه الاسم المستعار ملزم) عندما تتوفر بيانات موثوقة Torii،
4. الكاتب `artifacts/sorafs/portal.pin.report.json` والاختياري `portal.pin.proposal.json` و واصف قطع DNS وزمة ربط البوابة (`portal.gateway.binding.json` مع كتلة الرؤوس) حتى يبدأ تشغيل البراءات والشبكات التجريبية من مراجعة المعادلة دون قراءة سجلات CI.

اضبط تشغيل `PIN_ALIAS` و`PIN_ALIAS_NAMESPACE` و`PIN_ALIAS_NAME` و(اختياريا) `PIN_ALIAS_PROOF_PATH` قبل السكربت. استخدم `--skip-submit` للتجارب الجافة؛ يقوم بسير العمل في GitHub بتبديل ذلك عبر `perform_submit`.

## الخطوة 8 - نشر مواصفات OpenAPI وحزم SBOM

يتطلب DOCS-7 ان يمرر بناء البوابة ومواصفات OpenAPI و SBOM بنفس المسار الحتمي. تغطية الادوات الحالية الثلاثة:

1. **اعادة توليد وتوقيع المواصفات.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   استخدم `--version=<label>` لالتقاط اللقطة والتحكم (مثل `2025-q3`). يقوم المساعد بكتابة اللقطة الى `static/openapi/versions/<label>/torii.json` ويعكسها في `versions/current` ويحدث `static/openapi/versions.json` ببيانات SHA-256 وحالة المانيفست لخرائط التخطيط. موقع المطورين هذا مؤشر ليعرض النسخة ومعلومات البصمة/التوقيع. اذا تم حذف `--version` فسيفساء التسميات السابقة ويحدث `current` و`latest` فقط.

   يلتقط المانيفست بصمات الأصابع SHA-256/BLAKE3 حتى البوابة الجديدة من تدبيس `Sora-Proof` لـ `/reference/torii-swagger`.

2. **اصدار CycloneDX SBOMs.** يخطط مسار الاصدار SBOMs عبر syft حسب `docs/source/sorafs_release_pipeline_plan.md`. تحتفظ بالمخرجات قرب المصنوعات اليدوية:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **تغليف كل الحمولة داخل CAR.**

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

   اتبع نفس خطوات `manifest build` / `manifest sign` الخاصة بالموقع الرئيسي، وضبط الأسماء المستعارة لكل قطعة أثرية (مثل `docs-openapi.sora` للمواصفات و`docs-sbom.sora` لحزمة SBOM الموقعة). يبقي ذلك SoraDNS و GAR وتذاكر السماء مقيدة بكل حمولة.4. **الارسال والربط.** إعادة استخدام السلطة والـ Sigstoreحزمة، مع تسجيل الاسم المستعار tuple في قائمة الاصدار حتى يعرف المدققون اي اسم Sora يقابل اي بصمة.

يساعد في الحفاظ على تشغيل مانييفستات OpenAPI/SBOM مع بناء البوابة على ان يحتوي على كل تذكرة على مجموعة المعادلة الجزئية دون إعادة باكر.

### مساعد التمتة (CI/package script)

`./ci/package_docs_portal_sorafs.sh` يجمع الخطوات 1-8 في أمر واحد. يقوم بالمساعدة:

- إعدادات البوابة (`npm ci`، نوبات OpenAPI/Norito، أدوات السيولة).
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
- `--skip-build` - إعادة استخدام `docs/portal/build` الموجودة.
- `--skip-sync-openapi` - تجاوز `npm run sync-openapi` عندما لا يستطيع `cargo xtask openapi` الوصول الى crates.io.
- `--skip-sbom` - الاتصال `syft` عند عدم ضرورةه (مع تحذير).
- `--proof` - تنفيذ `sorafs_cli proof verify` لكل زوج CAR/manifest.
- `--sign` - تنفيذ `sorafs_cli manifest sign`.

للانتاج استخدم `docs/portal/scripts/sorafs-pin-release.sh`. جمع البوابة/OpenAPI/SBOM ويسجل المانيفستات ويسجل البيانات الوصفية النقية في `portal.additional_assets.json`. يدعم مفاتيح `--openapi-*` و`--portal-sbom-*` و`--openapi-sbom-*` لتعيين alias لكل قطعة أثرية وتجاوز حدود SBOM عبر `--openapi-sbom-source` وتخطي بعض الحمولات (`--skip-openapi`/`--skip-sbom`) `syft` غير افتراضي عبر `--syft-bin`.

السكر المحضرة كل الاوامر؛ انسخ السجل الى تذكرة الاصدار مع `package_summary.json` حتى خطوط الاتصال من مقارنة الملخصات والبيانات الوصفية دون تتبع خطوط الصدفة.

## الخطوة 9 - التحقق من البوابة + SoraDNS

قبل إعلان القطع، اثبت ان الاسم المستعار الجديد يحل عبر SoraDNS والبوابات تدبس البراهين الجديدة:

1. **تشغيل بوابة المسبار.** يقوم `ci/check_sorafs_gateway_probe.sh` ويسمح `cargo xtask sorafs-gateway-probe` على التركيبات في `fixtures/sorafs_gateway/probe_demo/`. للتشغيل الفعلي، توجيه الاختبار الى الـ hostname المطلوب:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   فيك مسبار رؤوس `Sora-Name` و`Sora-Proof` و`Sora-Proof-Status` وفق `docs/source/sorafs_alias_policy.md` ويفشل عند انحراف بصمة المانيفيست او TTLs او GAR Bindings.

   لإجراء فحوصات مفاجئة خفيفة الوزن (على سبيل المثال، عند ربط الحزمة فقط
   تم التغيير)، قم بتشغيل `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   يقوم المساعد بالتحقق من صحة حزمة الربط التي تم التقاطها وهو سهل الإصدار
   التذاكر التي تحتاج فقط إلى تأكيد ملزم بدلاً من تدريبات التحقيق الكاملة.

2. **جمع التدريبات المعادلة.** استخدم `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...` لجميع الرؤوس والسجلات في `artifacts/sorafs_gateway_probe/<stamp>/` وتحديث `ops/drill-log.md`، مع تشغيل خطافات التراجع او PagerDuty. اضبط `--host docs.sora` موضوع من مسار SoraDNS.

3. **التحقق من ارتباطات DNS.** عند نشر الدليل الخاص بالاسم المستعار، احفظ ملف GAR المشار إليه وارفق مع الاصدار المعادل. يمكن لمشغلي إعادة الاختبار عبر `tools/soradns-resolver`. قبل ارفاق JSON، نفّذ `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`. يمكن للمساعد ارسال ملخص `--json-out` بجانب GAR الموقّع.
  عند إعداد GAR جديد، الكبد `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`، ولا تستخدم `--manifest-cid` الا عند غياب ملف المانيفست. يقوم المساعد الان باشتقاق CID وبصمة BLAKE3 مباشرة من JSON ويزيل الاختلاف ويزيل تباينات التسميات ويرتبها ويضيف قوالب CSP/HSTS/Permissions-Policy قبل الكتابة والحتمية.

4. **مراقبة مقاييس مستعارة.** ابق `torii_sorafs_alias_cache_refresh_duration_ms` و`torii_sorafs_gateway_refusals_total{profile="docs"}` على الشاشة؛ طريقه معروض في `dashboards/grafana/docs_portal.json`.

## الخطوة 10 – المواجهة وحزم المنافس

- **اللوحات.** صدّر `dashboards/grafana/docs_portal.json` و`dashboards/grafana/sorafs_gateway_observability.json` و`dashboards/grafana/sorafs_fetch_observability.json` لكل اصدار وارفقة مع التذكرة.
- **مسبارات ارشفة.** تستخدم بـ `artifacts/sorafs_gateway_probe/<stamp>/` ضمن git-annex او مخزن البديل.
- **حزمة الاصدار.** خزن ملخصات CAR ومجموعات المانيفست وتواقيع Sigstore و`portal.pin.report.json` وسجلات Try-It وتقارير الروابط تحت مجلد واحد بتاريخ.
- **سجل تشغيل التدريبات.** عند التدريبات، يحدث ملف `scripts/telemetry/run_sorafs_gateway_probe.sh` `ops/drill-log.md` تتطلب متطلبات SNNet-5.
- **روابط الدخول.** ربط معرفات اللوحات Grafana او صادرات PNG مع مسار تقرير التحقيق حتى تطلب المراجعون من التحقق دون وصول Shell.

## الخطوة 11 - تمرين جلب مصادر متعددة ولوحة نتائج متساوية

ويلزم نشر متعدد SoraFS معادلة جلب المصدر (DOCS-7/SF-6) مع معادلة DNS/البوابة. بعد تثبيت المانيفيست:

1. **تشغيل `sorafs_fetch` على المانيفست الحي.** استخدم نفس الخطة/البيان مع بيانات معتمدة لكل البوابة الإلكترونية واحفظ جميع المخرجات:

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

   - احصل اولا على اعلانات المزود المشار إليها في المانيفيست (مثل `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) ومررها عبر `--provider-advert name=path` و تقييم الترشيح بشكل حتمي. استخدم `--allow-implicit-provider-metadata` **فقط** عند إعادة تشغيل التركيبات في CI.
   - عند وجود مناطق محمية، قم بتشغيل التشغيل المناسب للحصول على معادل لكل ذاكرة تخزين مؤقت/اسم مستعار.

2. **ارشفة المخرجات.** خزين `scoreboard.json` و`providers.ndjson` و`fetch.json` و`chunk_receipts.ndjson` في دليل المعادلة.

3. **تحديث القياسات.** استوردت المخرجات في اللوحة **SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`) وراقب `torii_sorafs_fetch_duration_ms`/`_failures_total`.

4. **متطلبات فحص التنبيه.** شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` وارفق مخرجات برومتوول.

5. **المدمج في CI.** فعّل خطوة `sorafs_fetch` عبر `perform_fetch_probe` في التدريج/الإنتاج والبديل الواضح.

## الترقية والملاحظة والتراجع

1. **الترقية:** يستخدم بـ أسماء مستعارة لـ التدريج والإنتاج. قم بالترقية عبر إعادة إعادة `manifest submit` بالمانيفست بنفسه مع تغيير `--alias-namespace/--alias-name` الى الاسم المستعار للإنتاج.
2. **المراقبة:** استخدم `docs/source/grafana_sorafs_pin_registry.json` ومسبارات البوابة الخاصة في `docs/portal/docs/devportal/observability.md`.
3. **التراجع:** لاعادة الحكمة، اعد النشرة المانيفست السابقة (او اسحب الاسم المستعار الحالي) عبر `sorafs_cli manifest submit --alias ... --retire`.

## قالب CI

يجب ان تشمل الحد الادني من خط الانابيب:

1. البناء + الوبر (`npm ci`، `npm run build`، توليد المجموع الاختباري).
2. المرسل (`car pack`) وحساب المانيفست.
3. التوقيع باستخدام OIDC الرمز الخاص بالوظيفة (`manifest sign`).
4. رفع الأصول (CAR، مانيفيست، حزمة، خطة، ملخصات).
5. الارسال الى رقم التعريف الشخصي:
   - سحب الطلبات -> `docs-preview.sora`.
   - العلامات / حماية -> ترقية إنتاج الاسم المستعار.
6. تشغيل المجسات + بوابات التحقق من الإثبات قبل الانتهاء.

`.github/workflows/docs-portal-sorafs-pin.yml` ربط هذه الخطوات لاالإصدارات اليدوية. يقوم الـ سير العمل بـ:

- بناء/اختبار البوابة،
- بناء التعبئة عبر `scripts/sorafs-pin-release.sh`،
- توقيع/تحقق من حزمة المانيفست عبر GitHub OIDC،
- رفع ملخصات CAR/manifest/bundle/plan/proof كـ artifacts،
- (اختياريا) مكتب المانيفست وربط الاسم المستعار عند توفر الأسرار.

اضبط الأسرار/المتغيرات التالية قبل التشغيل:

| الاسم | اللحوم |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | ضيف Torii الذي يكشف `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | معرف Epoch المسجل مع الارسال. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | طريقة التوقيع لارسال المانيفست. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | الاسم المستعار Tuple المربوط عند `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | إثبات الحزمة للاسم المستعار بترميز base64 (اختياري). |
| `DOCS_ANALYTICS_*` | التحليلات/المسبار نقاط الحالية. |

شغّل الـ سير العمل عبر واجهة الإجراءات:

1. زود `alias_label` (مثل `docs.sora.link`)، و`proposal_alias` اختياري، و`release_tag` اختياري.
2. اترك `perform_submit` غير مفعل ثم الاصول دون لمس Torii (dry run) او فتاحه للنشر المباشر على الاسم المستعار.

يبقى `docs/source/sorafs_ci_templates.md` كمرجع لقوالب عامة، لكن سير العمل الخاص بالبوابة هو المسار المفضل للاصدارات اليومية.

## قائمة التحقق

- [ ] نجاح `npm run build` و`npm run test:*` و`npm run check:links`.
- [ ] حفظ `build/checksums.sha256` و`build/release.json` ضمن الاصول.
- [ ] توليد CAR وخطة وملخص وملخص تحت `artifacts/`.
- [ ] حفظ Sigstore الحزمة والتوقيع المنفصل مع تسجيل.
- [ ] حفظ `portal.manifest.submit.summary.json` و`portal.manifest.submit.response.json` عند الارسال.
- [ ] ارشفة `portal.pin.report.json` (و`portal.pin.proposal.json` عند توفرها).
- [ ] حفظ السجلات `proof verify` و`manifest verify-signature`.
- [ ] تحديث اللوحات Grafana ونجاح تحقيقات Try-It.
- [ ] اختفاء ملاحظات النجوم (ID المانيفست السابق + Digest alias) مع تذكرة الاصدار.