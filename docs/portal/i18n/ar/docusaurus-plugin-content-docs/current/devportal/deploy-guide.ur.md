---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

##جائزہ

هناك نسخة احتياطية من البيانات **DOCS-7** (SoraFS شائعة) و **DOCS-8**
(تشغيل CI/CD) يمكن تشغيل بورتولبر بسهولة من خلال تقنية حديثة.
بناء/لينت رحلة، SoraFS پیكجنگ، Sigstore ذریعے مينى فيسٹ سينتنگ،
الاسم المستعار بروموشن، توثيق، والتراجع عن المعاينة والمعاينة
أطلق سراح فكرة رائعة ورائعة.

هل يفترض أن هذا هو الحال مع `sorafs_cli` باير (`--features cli`)
تم إصدار نسخة البناء) وتسجيل رقم التعريف الشخصي Torii لنقطة النهاية، و
Sigstore لـ OIDC إسناد. طول مدت راز (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`، Torii) و CI متجدد؛ لوكل رنز ان
صادرات شل ذات قيمة كبيرة.

## پيشگي قطاعي

- العقدة 18.18+ ساتھ `npm` أو `pnpm`.
- `sorafs_cli` جو `cargo run -p sorafs_car --features cli --bin sorafs_cli` حصل على ہو۔
- Torii URL `/v1/sorafs/*` ظهور الصفحة وأي بطاقة إلكترونية/رائعة موجودة هنا
  تم جمع شركاتي والأسماء المستعارة.
- مُصدر OIDC (إجراءات GitHub، GitLab، هوية عبء العمل)
  `SIGSTORE_ID_TOKEN` من هذا القبيل.
- اختيار: التشغيل الجاف لـ `examples/sorafs_cli_quickstart.sh` وGitHub/GitLab
  سير العمل إلى `docs/source/sorafs_ci_templates.md`.
- جرب ملف OAuth واير ايبلز (`DOCS_OAUTH_*`) وقم بإنشاء نسخة احتياطية
  پروموٹ کرنے سے پہلے [قائمة التحقق من تشديد الأمان](./security-hardening.md)
  چلايں. في حالة عدم وجود أي إبلز موجودًا، لن يتم تشغيل مقابض TTL/الاقتراع
  لا حدود للحجم الذي يمكنك من إنشاءه على الإطلاق؛ `DOCS_OAUTH_ALLOW_INSECURE=1`
  قم باستعراض المعاينات المنخفضة للتصدير. اختبار القلم هو شيء رائع
  منسلك كریں.

## رحلة 0 — Try it پروکسی بنڈل محفوظ کریں

Netlify أو البوابة للمعاينة بروموت کرنے پہلے جربها بروكس سورسز و
يحتوي على OpenAPI ملخص طبي رائع وهو لعبة ممتعة:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` مساعدات البروكس/المسبار/الاستعادة
تم تسجيل التوقيع OpenAPI و`release.json`
`checksums.sha256` لكهاتا. تم ترقية بوابة Netlify/SoraFS إلى Netlify/SoraFS
بطاقة الائتمان ريوورز وبروكس سورس وTorii أكبر حجم
إعادة بناء النسخة الجديدة من اللعبة. الهاتف المحمول أو بطاقة الائتمان التي يقدمها العميل
الحاملون فعالون (`allow_client_auth`) يتم وضع متطلبات خطة الطرح وCSP
رہیں۔

## رحلة 1 — پورٹل بناء ووبر

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

`npm run build` يعمل على `scripts/write-checksums.mjs` كلاتا، و
هذا هو الحل:

- `build/checksums.sha256` — `sha256sum -c` من أجل SHA256 ميني فيسٹ.
- `build/release.json` — ما هو (`tag`، `generated_at`، `source`)
  السيارة/المانيفست يمكن أن تكون جاتا.

تبرع بميزة Flea CAR التي ستساعدك على إنشاء نسخة جديدة من ريوورز
معاينة المقالة الفنية لفرق سكي.

## الخطوة 2 — أثاث اللعبة

CAR PICER COL Docusaurus هو منتج متعدد الاستخدامات. لا يوجد مثال على كل المقالات الفنية
`artifacts/devportal/` تحت الإنشاء.

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

عدد أجزاء JSON الصغيرة، والملخصات، وتلميحات التخطيط الإثباتي
`manifest build` و CI يستخدمان مرة أخرى بعد استخدام البطاقة.

## رحلةہ 2b — OpenAPI و SBOM معاون پيکج کریں

تم طلب DOCS-7 من قاعدة البيانات، وOpenAPI، وحمولات SBOM
أصبحت هذه البوابات شائعة الاستخدام في بوابات تكتيكية
`Sora-Proof`/`Sora-Content-CID` المزيد من السكاكين. ريليز ايلبر
(`scripts/sorafs-pin-release.sh`) الامتداد إلى OpenAPI (`static/openapi/`)
و`syft` تحتوي على SBOMs بالإضافة إلى `openapi.*`/`*-sbom.*` CARs.
البطاقة وما إلى ذلك `artifacts/sorafs/portal.additional_assets.json`
تسجيل الدخول . جهاز الطيران في الوقت المناسب، الحمولة الصافية للمراحل 2-4 د.
يتم استخدام البادئات وتسميات البيانات الوصفية كریں (على سبيل المثال
`--car-out "$OUT"/openapi.car` إلى `--metadata alias_label=docs.sora.link/openapi`).
تم تحديث ملف DNS إلى البيان/الاسم المستعار Torii إلى مسجل السجل (سات،
OpenAPI، پورٹل SBOM، OpenAPI SBOM) بوابة تاکہ شاع شاعہ آرٹیفیکٹس کے لئے
البراهين المدبسة

## رحلة 3 — صناعة السينما

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

روابط سياسة الدبوس التي يتم تثبيتها وفقًا لـ ٹيون كريں (على سبيل المثال
جزر الكناري کے لئے `--pin-storage-class hot`). JSON هي النسخة المتخصصة من مراجعة التعليمات البرمجية
کے لئے سہولت دیتا ہے.

## رحلة 4 — Sigstore سے ساين کریں

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

حزمة خلاصتي، هضم القطع، وOIDC ٹوكن كسجل تجزئة BLAKE3
كرتا بغیر JWT محفوظ کیے. حزمة وتوقيع منفصل دونو سنبھال کر
ركي؛ استخدام العروض الترويجية للإنتاج
شكرا جزيلا. مزود خدمة انترنت لوكل `--identity-token-env` بلس بلس
هذا (أو ماحول `SIGSTORE_ID_TOKEN` هو موقع ويب) متوافق مع OIDC
يلبر ٹوكن جارى.

## رحلة 5 — دبوس مسجل جمعة كرايز

لقد تم جمع هذا الفيديو (خطة القطعة) Torii التي تم جمعها بشكل كبير. ہميشہ ملخص الطلب
يمكن تسجيل الدخول المسجل/إثبات الاسم المستعار بسهولة.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority ih58... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

عند معاينة الاسم المستعار للكناري (`docs-preview.sora`)، يمكنك استخدام الاسم المستعار منفردًا
قم بالتقديم مرة أخرى من خلال تحسين إنتاج ضمان الجودة وتعزيز المواد التي يتم فحصها.

الاسم المستعار الملزم للرابط: `--alias-namespace`, `--alias-name`,
و`--alias-proof`۔ جورنس الاسم المستعار درخواست للطعام ہونے پر برهان حزمة (base64 أو
Norito بايت) عدد الملفات؛ هذه هي أسرار CI و`manifest submit`
كل ما سبق هو سبب تفضيلي للقراءة. كيف تنفق هذا الرقم من البلاستيك
لا يمكنك استخدام اسم DNS و DNS الذي تستخدمه في الاسم المستعار للملفات الفارغة.

## رحلة 5 ب — لعبة البروبوزل تاير كاراي

هذا هو فيلمي الذي جلس فيه البرلمان لتصنيع مشروع البروبوزل وهو ما يحدث الآن
سورا شار خاص بإسناده لبغير تبدويل بيش كر سكي. إرسال/توقيع المراحل ے
بعد التخفيض:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` كينونيكل `RegisterPinManifest`، قطعة دايت،
باليس، والاسم المستعار يلمح إلى محفوظ كرتا. إنها ٹکٹمجلة البرلمان
لا يُنصح بإعادة بناء حمولة الحمولة الإضافية بشكل ملحوظ
ديكو سكي. مفتاح السلطة Torii هو المفتاح الذي لا يوجد شيء آخر، أيضًا
لقد تم إنشاء شاري لوكل على شكل مشروع.

## رحلة 6 — الأدلة والإثباتات

التثبيت بعد الدرج ذو مراحل التحقق الحتمية:

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

- `torii_sorafs_gateway_refusals_total` و
  `torii_sorafs_replication_sla_total{outcome="missed"}` تحتوي على حالات شاذة.
- `npm run probe:portal` تم إجراء عملية المحاولة والتسجيل
  لا يوجد شيء مثبت على الإطلاق لخطأ ما.
- [النشر والمراقبة](./publishing-monitoring.md)
  لقد نجحنا في تمكين عملية DOCS-3c من نشر إمكانية الملاحظة في مراحلها التالية
  بورا ہو۔ ہیلپر اب متعدد `bindings` إدخالات (سائٹ، OpenAPI، پورٹل SBOM، OpenAPI)
  SBOM) قبول كرتا وأختيار `hostname` اسم المضيف
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` نافذ كرتا. نیچے والی الاحتجاج
  هذه هي حزمة الأدلة والأدلة JSON (`portal.json`، `tryit.json`، `binding.json`،
  و `checksums.sha256`) تبرع بالمزيد من المزايا:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## رحلة 6 أ — بوابة البداية

TLS SAN/خطة التحدي GAR صورة معززة لبوابة الاتصال و DNS
يجب أن يكون هذا هو الحال. آليبر DG-3 الجديدة تعكس الصورة
ہے، مضيفو أحرف البدل الأساسية، وشبكات SAN المضيفة الجميلة، وDNS-01، وجوجل
تحديات ACME شمار كرتا ہے:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON هو عبارة عن بطاقة بيانات متنقلة (أو بطاقة بيانات مسجلة)
بروتوكولات Torii و`torii.sorafs_gateway.acme` لشبكة SAN William William
و GAR ريورز التعيينات الأساسية/الجميلة التي تستضيف الاشتقاقات مرة أخرى
تصديق سكي. هذا هو ريليز بروموت لاحقة إضافية لـ `--name`
أسباب تنطوي على کریں.

## مرحہ 6ب — تعيينات المضيف الأساسية اخذ کریں

تم تحديث حمولات GAR إلى الاسم المستعار لرسم خرائط المضيف الحتمي
کریں۔ `cargo xtask soradns-hosts` و`--name` هو عنوان أساسي
(`<base32>.gw.sora.id`) بطاقة البدل المطلوبة (`*.gw.sora.id`) نكالتا
ہے، والمضيف الجميل (`<alias>.gw.sora.name`) أخذ کرتا ہے۔ ريليز آرتيفيكت
آخر التطورات في DG-3 تقديم GAR لريورز ورسم الخرائط بشكل منفصل
اسم:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

يتطلب توافق GAR أو بوابة ربط JSON الحاجة إلى المضيفين الذين يريدونك فورًا
تم استخدام رقم `--verify-host-patterns <file>`. ہیلپر متعدد
التحقق فلیں قبول کرتا ہے، جس سے ایك ہی استدعاء قالب GAR و
تدبيس `portal.gateway.binding.json` تدبيس الوبر بسهولة:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

DNS/البوابة ما هو ملخص JSON والتحقق من عدم وجود اتصال بالإنترنت
يمكن للمضيفين الأساسيين، وأحرف البدل، والمضيفين الجميلين التحقق من اشتقاقات المضيف مرة أخرى
شكرا جزيلا. تحتوي أيضًا الهواتف المحمولة على أسماء مستعارة جديدة تتيح لك إمكانية تكرارها
تم تنفيذ هذا الإجراء بعد ظهور تقرير GAR وتم اكتشاف مسار الأدلة.

## المرحلة 7 — موصف تحويل DNS

تم تجديد قطع الإنتاج التي أصبحت قابلة للاستبدال. كامياب
التقديم (الاسم المستعار ملزم) بعد `artifacts/sorafs/portal.dns-cutover.json`
نحن هنا، وهذا يشمل أو يشمل:- البيانات الوصفية المرتبطة بالاسم المستعار (مساحة الاسم/الاسم/الدليل، ملخص البيان، Torii URL،
  العصر المقدم، السلطة)؛
- سباق وسباق (علامة، تسمية الاسم المستعار، مسارات البيان/السيارة، خطة القطعة، حزمة Sigstore)؛
- مؤشرات التحقق (مسبار کمانڈ، الاسم المستعار + نقطة النهاية Torii)؛
- اختيار التحكم في التغيير (معرف التذكرة، نافذة التحويل، جهة اتصال العمليات،
  اسم مضيف الإنتاج/المنطقة)؛
- تدبيس `Sora-Route-Binding` واستخراج البيانات الوصفية لتعزيز المسار
  (المضيف الأساسي/CID، الرأس + مسارات الربط، أوامر التحقق)، حتى GAR
  التدريبات الترويجية والاحتياطية هي بمثابة حوالة؛
- تم إنشاء خطة الطريق آرٹیفیکٹس (`gateway.route_plan.json`، قوالب الرأس،
  ورؤوس التراجع المختلفة) لتغيير التذاكر وخطافات الوبر CI DG-3
  صورة من خطط الترويج/التراجع الأساسية التي تمكنك من الوصول إلى أعلى مستوى من السرعة؛
- اختیاری البيانات الوصفية لإبطال ذاكرة التخزين المؤقت (تطهير نقطة النهاية، متغير المصادقة، حمولة JSON،
  ومثال `curl` کمانڈ)؛ و
- تلميحات التراجع واصف الإرشادات (علامة الإصدار وملخص البيان)
  تتضمن الإشارة إلى تغيير التذاكر مسارًا احتياطيًا حتميًا شاملاً.

كيفية استخدام أداة تطهير ذاكرة التخزين المؤقت واستخدام واصف القطع المتعارف عليه بشكل أساسي
خطة البناء:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

حصل على `portal.cache_plan.json` وDG-3 بيك لبطاقة الائتمان منسل
قم بإضافة المضيفين/المسارات الحتمية (وتلميحات المصادقة المتعددة) و`PURGE`
الطلبات بشكل خاص. الواصف هو عبارة عن شريحة بيانات تعريف ذاكرة التخزين المؤقت المختصرة لهذا الغرض
حوالة دے سكتا ہے، جيس سے ريوورز للتحكم في التغيير ٹھیك نقاط النهاية للتوافق
لقد تم قطع عملية القطع أثناء التدفق.

تم أيضًا رفع صورة DG-3 الترويجية + قائمة التحقق من التراجع. إنه
`cargo xtask soradns-route-plan` تحديث ريوورز للتحكم في التغيير
الاسم المستعار لمراحل الاختبار المبدئي والتحويل والتراجع:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` مضيفون أساسيون/جميلون، فحص صحي منظم
إضافة، ربط GAR، تطهير ذاكرة التخزين المؤقت، وإجراءات التراجع محفوظات.
مقالة GAR/binding/cutover هي عبارة عن لعبة عملات البنك وهي مكتوبة
قدم مشقة وطعامًا رائعًا.

`scripts/generate-dns-cutover-plan.mjs` هو الواصف الذي يبدأ و
`sorafs-pin-release.sh` يعمل على كليتا. الجهاز جاهز للتكرار
أو تغيير اللون إلى:

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

دبوس المساعد چلانے سے پہلے بيانات التعريف الاختيارية لمتغيرات البيئة
أيضا:

| متغير | الغرض |
|----------|--------|
| `DNS_CHANGE_TICKET` | الواصف محفوظ ہونے ومعرف التذكرة۔ |
| `DNS_CUTOVER_WINDOW` | نافذة التحويل ISO8601 (مثال `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`، `DNS_ZONE` | اسم مضيف الإنتاج + المنطقة المعتمدة ۔ |
| `DNS_OPS_CONTACT` | الاسم المستعار عند الطلب أو اتصال التصعيد ۔ |
| `DNS_CACHE_PURGE_ENDPOINT` | يقوم الواصف بالتسجيل ومسح ذاكرة التخزين المؤقت لنقطة النهاية. |
| `DNS_CACHE_PURGE_AUTH_ENV` | تطهير الرمز المميز رکھنے و env var (الافتراضي: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | التراجع عن البيانات الوصفية هو المسار السابق واصف القطع. |

مراجعة تغيير نظام أسماء النطاقات (DNS) ملخص بيانات الموافقين على بيانات JSON، الاسم المستعار
الارتباطات، والتحقق من صحة بيانات CI والتحقق من صحة البيانات. أعلام CLI
`--dns-change-ticket`، `--dns-cutover-window`، `--dns-hostname`،
`--dns-zone`، `--ops-contact`، `--cache-purge-endpoint`،
`--cache-purge-auth-env` و`--previous-dns-plan` ويتجاوز حدود البطاقة
احصل على مساعد CI مع كليا جايا.

## رحلة 8 — الهيكل العظمي لملف منطقة الحل (اختيار)

تعرف على نافذة تحويل الإنتاج وتعرف على الهيكل العظمي لملف SNS Zonefile و
يتم استخدام مقتطف المحلل بشكل فعال. مطلوب سجلات DNS والبيانات الوصفية
متغيرات البيئة أو خيارات CLI التي تم تحديثها؛ واصف القطع المساعد
سيتم ذلك فورًا بعد `scripts/sns_zonefile_skeleton.py`. كم منكم
ملخص A/AAAA/CNAME ويليو و GAR (حمولة GAR الصافية مثل BLAKE3-256)
کریں۔ إذا كانت المنطقة/اسم المضيف معروفًا و`--dns-zonefile-out` فهذا هو السبب في ذلك
المساعد `artifacts/sns/zonefiles/<zone>/<hostname>.json` للدعم و
`ops/soradns/static_zones.<hostname>.json` هو مقتطف من محلل البيانات.

| المتغير / العلم | الغرض |
|-----------------|---------|
| `DNS_ZONEFILE_OUT`، `--dns-zonefile-out` | يحتوي هذا الهيكل العظمي على مسار لملف المنطقة. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`، `--dns-zonefile-resolver-snippet` | مسار مقتطف المحلل (إذا كان هذا هو الإعداد الافتراضي `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`، `--dns-zonefile-ttl` | تم الانتهاء من التسجيل عبر TTL (الافتراضي: 600 ثانية). |
| `DNS_ZONEFILE_IPV4`، `--dns-zonefile-ipv4` | عناوين IPv4 (بيئة مفصولة بفواصل أو علامة CLI قابلة للتكرار)۔ |
| `DNS_ZONEFILE_IPV6`، `--dns-zonefile-ipv6` | عناوين IPv6 ۔ |
| `DNS_ZONEFILE_CNAME`، `--dns-zonefile-cname` | اختیاری هدف CNAME۔ |
| `DNS_ZONEFILE_SPKI`، `--dns-zonefile-spki-pin` | دبابيس SHA-256 SPKI (base64). |
| `DNS_ZONEFILE_TXT`، `--dns-zonefile-txt` | اضافةی إدخالات TXT (`key=value`)۔ |
| `DNS_ZONEFILE_VERSION`، `--dns-zonefile-version` | إصدار ملف المنطقة المحسوب يسمح بتجاوز الكري. |
| `DNS_ZONEFILE_EFFECTIVE_AT`، `--dns-zonefile-effective-at` | بداية نافذة القطع `effective_at` الطابع الزمني (RFC3339) القسري. |
| `DNS_ZONEFILE_PROOF`، `--dns-zonefile-proof` | البيانات التعريفية هي دليل على التجاوز الحرفي. |
| `DNS_ZONEFILE_CID`، `--dns-zonefile-cid` | تجاوز البيانات الوصفية CID. |
| `DNS_ZONEFILE_FREEZE_STATE`، `--dns-zonefile-freeze-state` | حالة تجميد الحارس (لينة، صلبة، ذوبان، مراقبة، طوارئ)۔ |
| `DNS_ZONEFILE_FREEZE_TICKET`، `--dns-zonefile-freeze-ticket` | يجمد کے لئے مرجع تذكرة الوصي/المجلس۔ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`، `--dns-zonefile-freeze-expires-at` | ذوبان الجليد RFC3339 الطابع الزمني. |
| `DNS_ZONEFILE_FREEZE_NOTES`، `--dns-zonefile-freeze-note` | إضافةی ملاحظات تجميد (بيئة مفصولة بفواصل أو علامة قابلة للتكرار)۔ |
| `DNS_GAR_DIGEST`، `--dns-gar-digest` | حمولة GAR الموقعة BLAKE3-256 خلاصة (ست عشرية)۔ روابط البوابة ہونے پر لا بد۔ |

لا يزال بإمكانك متابعة سير عمل GitHub Actions أو أسرار مستودعات ويليز
يتم تشغيل دبوس الإنتاج على ملف Zonefile. درج ذیل الأسرار/القيم
زر الحفظ (السلاسل تحتوي على ملفات متعددة القيم مفصولة بفواصل وخطوط):

| سر | الغرض |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`، `DOCS_SORAFS_DNS_ZONE` | اسم المضيف/المنطقة المساعد للإنتاج. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | الواصف محفوظ الاسم المستعار عند الطلب۔ |
| `DOCS_SORAFS_ZONEFILE_IPV4`، `DOCS_SORAFS_ZONEFILE_IPV6` | شاع ہونے والے سجلات IPv4/IPv6۔ |
| `DOCS_SORAFS_ZONEFILE_CNAME` | اختیاری هدف CNAME۔ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | دبابيس Base64 SPKI ۔ |
| `DOCS_SORAFS_ZONEFILE_TXT` | إدخالات TXT إضافية ۔ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | هيكل عظمي محفوظ تجميد البيانات الوصفية. |
| `DOCS_SORAFS_GAR_DIGEST` | حمولة GAR الموقعة هي ملخص BLAKE3 المشفر بالسداسي. |

`.github/workflows/docs-portal-sorafs-pin.yml` چلاتے وقت `dns_change_ticket` و
مدخلات `dns_cutover_window` الواصف/ملف المنطقة مناسب لتغيير النافذة
البيانات الوصفية لے۔ إنها فارغة تمامًا من عمليات التشغيل الجافة.

الاستدعاء العام (دليل تشغيل المالك SN-7 يتوافق مع):

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

المساعد يقوم بتغيير التذكرة وإدخال TXT للجاتا والقطع
بداية النافذة `effective_at` الطابع الزمني الذي يرث كرتا منذ البداية
لا يوجد تجاوز. أكمل سير العمل
`docs/source/sorafs_gateway_dns_owner_runbook.md` د.

### ملاحظة DNS العامة

هيكل عظمي Zonefile هو المنطقة التي تتمتع بالسجلات الرسمية الممتعة. الإنترنت العام
نام سرورز من أجل تفويض المنطقة الأصلية لـ NS/DS سجل أو DNS
قبل كل شيء، الأمر ضروري.
- استخدام اختصار apex/TLD للاسم المستعار/ANAME (المخصص للموفر) أو البوابة
  أصبحت عناوين IP Anycast شائعة الاستخدام A/AAAA.
- سب ڈومينز کے لئے مضيف جميل مشتق (`<fqdn>.gw.sora.name`) کی طرف CNAME
  شاع کریں۔
- بوابة المضيف المتعارف عليه (`<hash>.gw.sora.id`) موجودة تحت القائمة والرابط
  منطقة بلك شائعة الاستخدام.

### البوابة الإلكترونية

نشر المساعد `portal.gateway.headers.txt` و`portal.gateway.binding.json`
هنا، هناك نموذجان DG-3 لمتطلبات ربط محتوى البوابة من خلال:

- `portal.gateway.headers.txt` مكمل كتلة رأس HTTP رکھتا ہے (جس میں
  `Sora-Name`، `Sora-Content-CID`، `Sora-Proof`، CSP، HSTS، و
  يشتمل واصف `Sora-Route-Binding` على) بوابات حافة الحافة والاستجابة
  لقد تم تدبيسها بشكل مستمر.
- `portal.gateway.binding.json` ومعلومات قابلة للقراءة آليًا
  قم بتغيير التذاكر وارتباطات المضيف/CID تلقائيًا
  هناك فرق كبير.

تعمل هذه الشبكة على `cargo xtask soradns-binding-template` وهي إصدار السجل والاسم المستعار وملخص البيان والبوابة
اسم المضيف هو الاسم المحفوظ `sorafs-pin-release.sh` الذي تم إنشاؤه.
قم بإعادة إنشاء كتلة الرأس أو تخصيص الصفحة للخلف:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

`--csp-template`، `--permissions-template`، أو `--hsts-template` الافتراضي
قوالب الرأس التي تتجاوز القواعد الخاصة بالنشر والتوجيهات الإضافية الإضافية؛
هناك مفاتيح `--no-*` متاحة تعمل على إغلاق كل رأس بشكل كامل.

مقتطف الرأس لطلب تغيير CDN الذي يربط بين منسلك و JSON
خط أنابيب أتمتة البوابة يقدم دليلًا لترويج المضيف الأساسي
هناك ميل.

مساعد xtask اب الكنسي راستہ ہے۔ ريليز سكربت مساعد التحقق
في ما يتعلق بهذا DG-3، هناك أدلة حديثة تتضمن الكثير. يمكنك أيضًا ربط JSON
هذا الجهاز للتغيير هو التالي:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

يتم تدبيس الحمولة النافعة `Sora-Proof` التي تحتوي على حمولة كاملة، `Sora-Route-Binding`
البيانات التعريفية هي عبارة عن CID + اسم المضيف الذي يحمل اسم المضيف، وفي حالة انحراف الرأس
هل تفشل بشكل فوري؟ يمكنك أيضًا استخدام CI لتتمكن من الحصول على المزيد من المعلومات
نشر أفكار أخرى ستساعدك على نشر مقالات DG-3
تم التحقق من صحة القطع الملزم.> **تكامل واصف DNS:** `portal.dns-cutover.json` اب `gateway_binding`
> تتضمن المجموعة كرتا وهو عبارة عن (المسارات، والمحتوى CID، وحالة الإثبات، و
> قالب الرأس الحرفي) نصيحة اشار كرتا **اور** `route_plan` مقطع
> `gateway.route_plan.json` وقوالب رأس التراجع الرئيسية + حوالة ديتا.
> تحتوي تذكرة تغيير DG-3 على ميزة أو ميزة إضافية تتضمن تقنية ريوورز هذه
> `Sora-Name`/`Sora-Proof`/`CSP` فرق سكاكين وفحص سكاكين
> حزمة أدلة ترويج/التراجع عن المسار متوافقة مع بعضها البعض
> بناء الأرشيف کھولے۔

## رحلةہ 9 — شاشات النشر چلائیں

هذا المقال **DOCS-3c** يحتوي على سلسلة من الأدلة، جربه بروكس،
يتم ربط روابط البوابة بعد ذلك. المراحل 7-8 فوراً بعد توحيدها
قم بمراقبة الشاشات وهي عبارة عن مجسات مجدولة للأسلاك:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` config ملف تسجيل الدخول (المخطط الموضح
  `docs/portal/docs/devportal/publishing-monitoring.md` (محدد) وعمليات التحقق هذه
  كلتا ہے: تحقيقات مسار البوابة + التحقق من صحة سياسة CSP/Permissions، جرب الوكيل
  المجسات (اختاري لـ `/metrics` نقطة النهاية)، وأداة التحقق من ربط البوابة
  (`cargo xtask soradns-verify-binding`) الاسم المستعار/الشيكات الواضحة
  Sora-Content-CID موجود + المتوقع ويليو في وقت سابق من هذا العام.
- إذا كان هناك مسبار آخر، فستتمكن من التحكم في نقاط غير الصفر وCI، ووظائف cron،
  أو الأسماء المستعارة لمشغلي دليل التشغيل
- `--json-out` ا واحد ملخص JSON payload لکھتا ہے جس میں في حالة الهدف ہے؛
  `--evidence-dir` `summary.json`، `portal.json`، `tryit.json`، `binding.json`، و
  `checksums.sha256` شاشة العرض وشاشات ريوورز بغداد الجديدة
  النتائج تختلف. الشركة المصنعة `artifacts/sorafs/<tag>/monitoring/`
  حزمة Sigstore واصف تحويل DNS المتوفران في مجلة كريكايو.
- مراقبة آؤٹ پٹ، تصدير Grafana (`dashboards/grafana/docs_portal.json`)، و
  معرف تدريب Alertmanager الذي يتضمن المزيد من التفاصيل حول DOCS-3c SLO بعدي
  آآآآآآآآآآآآآآآآآآآآآآآآه دليل اللعب المخصص لمراقبة النشر:
  `docs/portal/docs/devportal/publishing-monitoring.md`۔

تحقق البوابة من HTTPS المطلوب و`http://` عناوين URL الموضحة أدناه
لا يوجد تكوين لشاشة `allowInsecureHttp`؛ أهداف الإنتاج / التدريج
يقوم TLS باستعراض وتجاوز المعاينات المنخفضة فقط لتنشيط النشاط.

تم تحديث البوابة بعد `npm run monitor:publishing` من Buildkite/cron
أتمتة کریں۔ إنه أمر رائع، عندما يتم الإشارة إلى عناوين URL للإنتاج، ويسهل عليك الأمر بعض الشيء
صدرت نتيجة لـ SRE/Docs عن حصار حصار مرضي.

## `sorafs-pin-release.sh` ثابت آلي

`docs/portal/scripts/sorafs-pin-release.sh` المراحل 2-6 وتغليف الكرتا. هو:

1. `build/` هي عبارة عن كرة قطران حتمية،
2. `car pack`، `manifest build`، `manifest sign`، `manifest verify-signature`،
   و `proof verify` شلاتا ہے،
3. بيانات اعتماد Torii متاحة ويمكنك الاختباء على `manifest submit` (اسم مستعار للربط)
   چلاتا ہے، و
4. `artifacts/sorafs/portal.pin.report.json`, اختیاری `portal.pin.proposal.json`,
  واصف تحويل DNS (عمليات الإرسال اللاحقة)، وحزمة ربط البوابة
  (`portal.gateway.binding.json` + كتلة رأس النص) للحوكمة،
  الشبكات، وعمليات CI لا تتضمن سوى حزمة أدلة منفصلة عن الأخرى.

`PIN_ALIAS`، `PIN_ALIAS_NAMESPACE`، `PIN_ALIAS_NAME`، و (اختیاری)
`PIN_ALIAS_PROOF_PATH` سيت كري. يتم استخدام التشغيل الجاف لـ `--skip-submit`؛
لم يتم إدخال سير عمل GitHub إلا عبر `perform_submit`.

## مرحہ 8 — مواصفات OpenAPI وحزم SBOM شاع کریں

تم طلب DOCS-7 من خلال بناء الصفحة ومواصفات OpenAPI ومقالة SBOM النموذجية
خط الأنابيب الحتمي سے گزریں۔ يوجد مساعدين جدد للبطاقة:

1. **المواصفات الجديدة للطائرات والسفن.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   هذه اللقطة التاريخية مهمة جدًا في `--version=<label>` وهي ذريعة ريليز
   ليبل فراهام كريں (مثلاً `2025-q3`). لقطة مساعد کو
   `static/openapi/versions/<label>/torii.json` موجود، وهو `versions/current`
   تعكس البيانات والبيانات الوصفية (SHA-256، حالة البيان، الطابع الزمني المحدث)
   `static/openapi/versions.json` سجل بياناتك. كل من مجلة ويلبر أو مؤشر الفهرس
   قم باستخدام منتقي إصدارات Swagger/RapiDoc Pinels وهو عبارة عن سكاكين وما يتعلق بالهضم/التوقيع
   معلومات مضمنة پیش کر سکیں. `--version` هذا هو الحل الأمثل لمشكلة بلبل
   لقد تم إنفاق مؤشرات `current` + `latest` بشكل سريع.

   ملخص البيان SHA-256/BLAKE3 محفوظات كرتا وبوابة تاکہ `/reference/torii-swagger`
   لئے `Sora-Proof` رؤوس تدبيس.

2. **CycloneDX SBOMs خارج الشبكة. ** يتم توقع خط أنابيب آخر من SBOMs المستندة إلى syft.
   تم تصميمه بواسطة `docs/source/sorafs_release_pipeline_plan.md` في درج واحد.
   قم ببناء القطع الأثرية مرة أخرى:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. ** الحمولة الصافية التي يمكن حملها بالسيارة.**

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

   هذه هي مراحل العمل `manifest build` / `manifest sign`، و
   هناك أثاث للأسماء المستعارة ٹيون كريں (مثل المواصفات لـ `docs-openapi.sora` و
   يحتوي على كابل SBOM `docs-sbom.sora`). گ الأسماء المستعارة رکھنے سے إثباتات SoraDNS،
   GARs، والتراجع هو نطاق خاص للحمولة النافعة.

4. **جمع البيانات وربطها.** والسلطة + Sigstore استخدام الحزمة مرة أخرى،
   تحتوي القائمة المرجعية الخاصة بالريليز على الاسم المستعار لصف التسجيل الذي يستخدمه هواة آخرون
   سورا نام كس الملخص الواضح سے منسلک ہے۔

تُظهِر المواصفات/SBOM كيفية إنشاء نسخة محمولة وهي عبارة عن اقتراح جديد تمامًا
يحتوي ريليز على مجمل الموقع الإبداعي الموجود ويعيد تعبئة باكر مرة أخرى.

### آٹومیشن ہیلبر (CI / البرنامج النصي للحزمة)

`./ci/package_docs_portal_sorafs.sh` المراحل 1-8 لتشفير الكرتا وتجميع البيانات
هذا الحيوان **DOCS-7** هو عبارة عن أداة رائعة لتعلم اللغة الإنجليزية. ايلبر:

- مطلوب پورٹل الإعدادية چلاتا (`npm ci`، OpenAPI/norito sync، اختبارات القطعة)؛
- بورتل، OpenAPI، وSBOM CARs + أزواج البيان `sorafs_cli` التي تم إصدارها؛
- اختير على `sorafs_cli proof verify` (`--proof`) وتوقيع Sigstore
  (`--sign`، `--sigstore-provider`، `--sigstore-audience`) چلاتا ہے؛
- جميع الأفكار الفنية لـ `artifacts/devportal/sorafs/<timestamp>/` التي تحت الماء و
  `package_summary.json` لإخراج أدوات CI/الإصدار، استيعاب القرص الصلب؛ و
- `artifacts/devportal/sorafs/latest` أحدث نصيحة للكتابة الحديثة.

مثال (Sigstore + PoR خط الأنابيب الكامل):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

أعلام قابلة للتوجيه:

- `--out <dir>` – تجاوز الجذر الاصطناعي (الطابع الزمني الافتراضي مجلدات رکھتا ہے).
- `--skip-build` – `docs/portal/build` موجود مرة أخرى (على CI عبر الإنترنت)
  المرايا لا يمكن إعادة بنائها).
- `--skip-sync-openapi` – `npm run sync-openapi` ما هو مطلوب من `cargo xtask openapi`
  crates.io لا يوجد أي شيء آخر.
- `--skip-sbom` – لا يوجد رقم `syft` للهاتف المحمول (تحذير سكربت).
- `--proof` – يتم استخدام CAR/manifest `sorafs_cli proof verify`. ملف متعدد
  الحمولات الصافية الرئيسية لـ CLI تحتوي على مخطط رياضي مقسم، إذا `plan chunk count`
  هناك أخطاء في ربط المفتاح وبوابة المنبع في جهاز البحث.
- `--sign` – `sorafs_cli manifest sign` چلايں. تم إنشاء `SIGSTORE_ID_TOKEN`
  (أو `--sigstore-token-env`) أو CLI إلى `--sigstore-provider/--sigstore-audience`
  لقد جلبت له ذرية.

يتم استخدام مصنوعات الإنتاج `docs/portal/scripts/sorafs-pin-release.sh`.
هذا اب بورت، OpenAPI، وحمولات SBOM پيك كرتا، وبيان پرس كرتا،
ويحتوي `portal.additional_assets.json` على بيانات تعريف إضافية لتسجيل البيانات.
المساعد والمقابض المختيرة ومفتاح استخدام أداة تغليف CI، لا يزال جديدًا
`--openapi-*`، و`--portal-sbom-*`، و`--openapi-sbom-*` يتم التبديل إليها أيضًا.
اثاث في الاسم المستعار tuples مقرر دراسي، `--openapi-sbom-source` SBOM
مأخذ تجاوز كر سکيں، خصوصية الحمولات کھوڑ سکیں (`--skip-openapi`/`--skip-sbom`)،
والرقم غير الافتراضي `syft` هو طرف الكمبيوتر المحمول `--syft-bin`.

إنه سكربت متقن وجميل وجميل؛ سجل `package_summary.json` متصل
يتضمن الريليز ملخصات ريورز CAR، والبيانات الوصفية للخطة، وحزمة Sigstore
التجزئات هي عبارة عن قذيفة مخصصة منفصلة.

## مرحہ 9 — Gateway + SoraDNS کی تصدیق

قطع هو إعلان لقاعدة بيانات ثابتة جديدة باسم مستعار SoraDNS تم حله
الأدلة والبوابات تدبيس الأدلة الآن:

1. **بوابة التحقيق.** `ci/check_sorafs_gateway_probe.sh` تركيبات مختلفة
   `cargo xtask sorafs-gateway-probe` چلاتا ہے جو
   `fixtures/sorafs_gateway/probe_demo/` موجود. عمليات النشر الحقيقية
   التحقيق في اسم المضيف في كلايد:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   المسبار `Sora-Name`، `Sora-Proof`، و`Sora-Proof-Status`
   `docs/source/sorafs_alias_policy.md` يتوافق مع فك التشفير وإذا كان ملخص البيان،
   يمكن أن تنجرف روابط TTL أو روابط GAR إلى مكان آخر.

   لإجراء فحوصات مفاجئة خفيفة الوزن (على سبيل المثال، عند ربط الحزمة فقط
   تم التغيير)، قم بتشغيل `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   يقوم المساعد بالتحقق من صحة حزمة الربط التي تم التقاطها وهو سهل الإصدار
   التذاكر التي تحتاج فقط إلى تأكيد ملزم بدلاً من تدريبات التحقيق الكاملة.

2. ** مجموعة أدلة الحفر. ** تدريبات السبر أو تشغيل PagerDuty الجاف للمسبار
   `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- …`
   وهو يلف الكري. رؤوس المجمع/السجلات کو
   `artifacts/sorafs_gateway_probe/<stamp>/` مكرر، `ops/drill-log.md` مفعل
   الكرتا و (اختياري) خطافات التراجع أو حمولات PagerDuty هي أيضًا خطاف التراجع. سورا دي إن إس
   لم يتم التحقق من صحة `--host docs.sora` حتى رمز IP الثابت.3. ** ربطات DNS هي دليل على الأسماء المستعارة.** يمكن استخدام دليل الاسم المستعار لـ جب جورنس في التحقيق
   حوالة ديا جيا فايل (`--gar`) تسجيل الدخول والحصول على الأدلة التي ستساعدك في اللعب.
   تم العثور على المحلل المالي `tools/soradns-resolver` في شريط البحث
   لا تمثل الإدخالات المخزنة مؤقتًا بيانًا للشرف. JSON منسلك سے پہلے چلايں
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   هذا هو تعيين المضيف الحتمي، والبيانات الوصفية الواضحة، وتسميات القياس عن بعد في وضع عدم الاتصال
   التحقق من صحة ہو جايں۔ وقع المساعد GAR کے ساتھ `--json-out` ملخص نكال سكتا ہے
   حصل جميع المراجعين على دليل ثنائي قابل للتحقق.
  نيا GAR بنات وقت طويل
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (`--manifest-cid <cid>` تم إنفاقه على قائمة الأجهزة غير المستخدمة). مساعد اب
  CID **اور** BLAKE3 ملخص دونوں البيان JSON سے راست نکالتا، مسافة بيضاء
  تقليم كرتا ہے، دہرے `--telemetry-label` أعلام ختم كرتا ہے، تسميات فرز كرتا ہے، و
  JSON يدعم قوالب CSP/HSTS/Permissions-Policy الافتراضية
  الحمولة الحتمية هي عبارة عن تسميات مختلفة للأصداف.

4. **مقاييس الاسم المستعار للتقييم.** `torii_sorafs_alias_cache_refresh_duration_ms`
   و`torii_sorafs_gateway_refusals_total{profile="docs"}` يقوم بالتحقيق في الأمر
   سكرين بركات؛ سلسلة دونو `dashboards/grafana/docs_portal.json` تحتوي على لوحة.

## رحلةہ 10 — المراقبة وتجميع الأدلة

- **لوحات المعلومات.** ہر ریلیز کے لئے `dashboards/grafana/docs_portal.json` (SLOs للمدخل)،
  `dashboards/grafana/sorafs_gateway_observability.json` (زمن استجابة البوابة + صحة الإثبات)،
  و`dashboards/grafana/sorafs_fetch_observability.json` (صحة المنسق)
  تصدير کریں۔ يقوم JSON بتصدير عدد كبير من سلاسل ألعاب الفيديو إلى ريوورز
  Prometheus استعلامات جديدة.
- **أرشيفات التحقيق.** `artifacts/sorafs_gateway_probe/<stamp>/` git-annex أو
  لا يوجد دلو من الأدلة. ملخص التحقيق، والعناوين، والبرنامج النصي للقياس عن بعد سے
  تتضمن حمولة PagerDuty حمولة.
- **حزمة الإصدار.** پورٹل/SBOM/OpenAPI ملخصات CAR، حزم البيان، Sigstore
  التوقيعات، `portal.pin.report.json`، وسجلات اختبار Try-It، وتقارير التحقق من الارتباط
  مجلد ذو طابع زمني (مثال `artifacts/sorafs/devportal/20260212T1103Z/`) مكرر.
- **سجل الحفر.** قم بفحص مجسات الحفر
  `scripts/telemetry/run_sorafs_gateway_probe.sh` و `ops/drill-log.md` يتم إلحاقه
  هذه التقنية والأدلة تتطلب فوضى SNNet-5.
- **روابط التذاكر.** قم بتغيير معرفات اللوحة Grafana أو صادرات PNG المرفقة.
  حوالة يومية، ومسار تقرير التحقيق يشمل أيضًا القراءة، حيث يمكن للمراجعين الوصول إلى الصدفة
  قم بالتحقق من SLOs بشكل متقاطع.

## رحلةہ 11 — تدريب جلب متعدد المصادر وأدلة لوحة النتائج

SoraFS تمت المطالبة بجلب أدلة متعددة المصادر (DOCS-7/SF-6)
وقد تم بالفعل إثبات إثباتات DNS/البوابة. دبوس البيان بعد:

1. **البيان المباشر للخلاف `sorafs_fetch` التصريح.** المراحل 2-3 الخطة/البيان
   استخدام بيانات اعتماد البوابة الافتراضية وموفر الخدمة. ہر
   بعض العناصر المهمة التي يقوم بها المنسقون في مسار القرار لإعادة تشغيل اللعبة:

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

   - بيان حوالة دي گئي إعلانات الموفر پہلے کریں (مثلا)
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     و`--provider-advert name=path` تلتقط لوحة النتائج
     نوافذ القدرة الحتمية تقييم كر .
     `--allow-implicit-provider-metadata` **s** CI إعادة تشغيل المباريات في الوقت المحدد
     استخدام کریں؛ يتم تثبيت تدريبات الإنتاج على الإعلانات الموقعة والإعلانات الموقعة عليها.
   - مناطق إضافية واضحة تشير إلى اهتماماتك بصفوف موفر الخدمة
     قم بإدارة ذاكرة التخزين المؤقت/الاسم المستعار لمطابقة جلب القطعة الأثرية.

2. **آخر المقالات.** `scoreboard.json`، `providers.ndjson`، `fetch.json`، و
   `chunk_receipts.ndjson` لاستعراض الأدلة التي تستخدمها. أو نظير
   الترجيح، وإعادة محاولة الميزانية، وزمن الوصول EWMA، والإيصالات لكل قطعة
   بدأت حرب الپيكات SF-7 في الظهور.

3. **اللعبة الإلكترونية.** جلب المخرجات **SoraFS إمكانية ملاحظة الجلب**
   بورصة بور (`dashboards/grafana/sorafs_fetch_observability.json`) ميناء كارى،
   `torii_sorafs_fetch_duration_ms`/`_failures_total` ولوحات نطاق الموفر
   الشذوذ. لقطات لوحة Grafana ستساعدك على تحديد مسار لوحة النتائج
   لينك كریں.

4. **قواعد التنبيه للتدخين.** `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   ما الذي يجب فعله هو التحقق من صحة حزمة التنبيه Prometheus. com.promtool
   الكثير من الأشياء التي توقف تسجيل مستندات DOCS-7 لسجلات اللعبة و
   تنبيهات المزود البطيء مسلح رہیں۔

5. **سلك سلك CI.** سير عمل دبوس المنفذ `sorafs_fetch` رحلة `perform_fetch_probe`
   تم إدخال الإدخال؛ يعد التدريج/الإنتاج عملية جلب فعالة
   حزمة بيان الأدلة التي تم إجراؤها منذ فترة طويلة. تدريبات لوكل و الاسكربت
   استخدام بروتوكول تصدير الرموز المميزة للبوابة و`PIN_FETCH_PROVIDERS`
   قائمة مقدمي الخدمات مفصولة بفواصل

## پروموشن، وقابلية الملاحظة، والتراجع

1. **بروموشن:** التدريج والإنتاج يحتوي على أسماء مستعارة. پروموشن کے لئے
   هذا البيان/الحزمة هو `manifest submit` مرة أخرى و
   `--alias-namespace/--alias-name` هو الاسم المستعار للإنتاج پر سوئچ کریں. هو سے سؤال وجواب
   لا يلزم وجود دبوس التدريج بعد إعادة البناء أو إعادة التوقيع.
2. **المراقبة:** سجل الدبوس
   (`docs/source/grafana_sorafs_pin_registry.json`) و تحقيقات بورٹل مخصوص
   (`docs/portal/docs/devportal/observability.md` رقم) بطاقة الشحن. الانجراف الاختباري,
   تحقيقات فاشلة، أو دليل على ارتفاعات إعادة المحاولة للتنبيه.
3. **التراجع:** إعادة إرسال البيان مرة أخرى (أو الاسم المستعار الموجود)
   التقاعد) `sorafs_cli manifest submit --alias ... --retire` . شكرا
   آخر حزمة معروفة وجيدة وملخص CAR
   أدلة التراجع جديدة.

## قالب سير عمل CI

ما هو خط الأنابيب الذي يمكنك القيام به:

1. البناء + الوبر (`npm ci`، `npm run build`، إنشاء المجموع الاختباري).
2. الحزمة (`car pack`) وبيانات الحساب الحسابي.
3. علامة تسجيل OIDC الخاصة بنطاق الوظيفة (`manifest sign`).
4. تدقيق المصنوعات اليدوية (السيارة، البيان، الحزمة، الخطة، الملخصات).
5. دبوس التسجيل يمكن إرساله:
   - سحب الطلبات → `docs-preview.sora`.
   - العلامات / الفروع المحمية → ترويج الاسم المستعار للإنتاج.
6. خروج مجسات سے پہلے + بوابات التحقق من الإثبات.

`.github/workflows/docs-portal-sorafs-pin.yml` الإصدارات اليدوية كلها كاملة
مراحل جوڑتا ہے۔ سير العمل:

- بناء/اختبار البورتال کرتا ہے،
- `scripts/sorafs-pin-release.sh` تم إنشاء پيکج كرتا،
- GitHub OIDC استخدم حزمة البيان سنين/التحقق من كرتا،
- ملخصات السيارة/البيان/الحزمة/الخطة/الإثبات والعناصر التي يتم تنزيلها، و
- (اختیاری) الأسرار موجودة ويمكن بيانها + ربط الاسم المستعار بتقديم كرتا ہے۔

المهمة التي تم الانتهاء منها في أسفل ملف أسرار/متغيرات المستودع:

| الاسم | الغرض |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | Torii host جو `/v1/sorafs/pin/register` ظہر کرتا ہے۔ |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | التقديمات عبارة عن تسجيل مستمر ومعرف العصر. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | التقديم الواضح کے لئے سلطة التوقيع. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | الاسم المستعار Tuple جو `perform_submit` صحيح ہونے پر مانيف سے ربط ہوتا ہے۔ |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | حزمة إثبات الاسم المستعار المشفرة بـ Base64 (اختيار؛ الاسم المستعار المرتبط بـ Ē لئے حذف کریں). |
| `DOCS_ANALYTICS_*` | يمكن إعادة استخدام التحليلات/نقاط نهاية الاختبار المتوفرة لسير العمل بشكل متكرر. |

واجهة مستخدم الإجراءات - مشغل سير العمل:

1. `alias_label` بطاقة الائتمان (مثلاً `docs.sora.link`)، `proposal_alias` اختياري،
   وتجاوز `release_tag` اختياري.
2. المصنوعات اليدوية التي تم إنشاؤها `perform_submit` والتي لم يتم تحديدها (Torii التي تعمل باللمس)
   أو يقوم بتمكين الاسم المستعار الذي تم تكوينه للنشر.

`docs/source/sorafs_ci_templates.md` هو الريبو الذي سيستمر في الترويج له
تساعد مساعدات CI العامة في تحسين سير العمل، مما يؤدي إلى تحسين سير العمل.

## قائمة المراجعة

- [ ] `npm run build` و`npm run test:*` و`npm run check:links`.
- [ ] `build/checksums.sha256` و `build/release.json` المصنوعات اليدوية محفوظه.
- [ ] السيارة والخطة والبيان والملخص `artifacts/` تحت القائمة.
- [ ] حزمة Sigstore + علامة التوقيع المنفصلة محفوظه.
- [ ] `portal.manifest.submit.summary.json` و`portal.manifest.submit.response.json`
      التقديمات كانت محفوظ ہوئے۔
- [ ] `portal.pin.report.json` (واختیاری `portal.pin.proposal.json`)
      CAR/المصنوعات اليدوية الواضحة موجودة بالفعل.
- [ ] `proof verify` و `manifest verify-signature` لاگز آركاويو ہوئے.
- [ ] Grafana لوحات المعلومات + اختبار Try-It.
- [ ] ملاحظات التراجع (معرف البيان السابق + ملخص الاسم المستعار)