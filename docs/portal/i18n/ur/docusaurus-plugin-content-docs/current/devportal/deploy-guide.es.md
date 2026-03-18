---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## جائزہ

یہ پلے بوک روڈ میپ عناصر کو تبدیل کرتی ہے ** دستاویزات -7 ** (ریلیز SoraFS) اور ** دستاویزات -8 **
(CI/CD پن آٹومیشن) ڈویلپر پورٹل کے لئے قابل عمل طریقہ کار میں۔
بلڈ/لنٹ مرحلے ، SoraFS پیکیجنگ کا احاطہ کرتا ہے ، Sigstore کے ساتھ ظاہر ہوتا ہے ،
عرف پروموشن ، توثیق اور رول بیک مشقیں تاکہ ہر پیش نظارہ اور ریلیز
تولیدی اور قابل آڈٹیبل بنیں۔

بہاؤ یہ فرض کرتا ہے کہ آپ کے پاس `sorafs_cli` بائنری ہے (`--features cli` کے ساتھ بنایا گیا ہے) ، تک رسائی
Torii PIN-REGISTIRY اجازت کے ساتھ اختتامی نقطہ ، اور OIDC اسناد Sigstore کے لئے۔ راز رکھیں
آپ کے CI والٹ میں طویل عرصے تک (`IROHA_PRIVATE_KEY` ، `SIGSTORE_ID_TOKEN` ، Torii ٹوکن) ؛
مقامی پھانسی ان کو شیل برآمدات سے لوڈ کرسکتی ہے۔

## شرائط

- `npm` یا `pnpm` کے ساتھ نوڈ 18.18+۔
- `sorafs_cli` `cargo run -p sorafs_car --features cli --bin sorafs_cli` سے۔
- Torii URL جو `/v1/sorafs/*` کے علاوہ ایک اتھارٹی اکاؤنٹ/نجی کلید کو بے نقاب کرتا ہے جو ظاہر اور عرفی نام بھیج سکتا ہے۔
- OIDC جاری کرنے والا (گٹ ہب ایکشنز ، گٹ لیب ، ورک بوجھ کی شناخت وغیرہ) `SIGSTORE_ID_TOKEN` جاری کرنے کے لئے۔
- اختیاری: خشک رنز کے لئے `examples/sorafs_cli_quickstart.sh` اور Github/gitlab ورک فلو سہاروں کے لئے `docs/source/sorafs_ci_templates.md`۔
- کوشش کریں کہ اس کی تشکیل کریں
  [سیکیورٹی سخت کرنے والی چیک لسٹ] (./security-hardening.md) کسی تعمیر کو فروغ دینے سے پہلے
  لیب پورٹل بلڈ کے باہر اب ناکام ہوجاتا ہے جب یہ متغیرات غائب ہوں
  یا جب ٹی ٹی ایل/پولنگ نوبس اطلاق شدہ ونڈوز سے باہر ہوں۔ برآمد
  صرف ڈسپوز ایبل مقامی پیش نظارہ کے لئے `DOCS_OAUTH_ALLOW_INSECURE=1`۔ منسلک کریں
  رہائی کے ٹکٹ کے قلم ٹیسٹ کے ثبوت۔

## مرحلہ 0 - پراکسی سے ایک پیکٹ پر قبضہ کریں اسے آزمائیں

نیٹ لائی یا گیٹ وے کے پیش نظارہ کو فروغ دینے سے پہلے ، پراکسی ذرائع کو آزمائیں اور اس پر مہر لگائیں
منشور کی ہضم OpenAPI ایک عزم پیکیج میں دستخط شدہ:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` کاپی پراکسی/تحقیقات/رول بیک مددگار ، دستخط کی تصدیق کرتا ہے
OpenAPI اور `release.json` پلس `checksums.sha256` لکھیں۔ اس پیکیج کو ٹکٹ سے منسلک کریں
نیٹ لیف/SoraFS گیٹ وے کی تشہیر تاکہ جائزہ لینے والے عین مطابق ذرائع کو دوبارہ پیش کرسکیں
بغیر کسی تعمیر نو کے پراکسی اور ہدف Torii کے پٹریوں کی۔ پیکیج میں یہ بھی ریکارڈ کیا گیا ہے کہ آیا
منصوبے کو برقرار رکھنے کے لئے کسٹمر سے فراہم کردہ بیئررز کو فعال کیا گیا (`allow_client_auth`)
ہم آہنگی میں رول آؤٹ اور سی ایس پی کے قواعد۔

## مرحلہ 1 - پورٹل کی تعمیر اور لنٹ

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

`npm run build` خود بخود `scripts/write-checksums.mjs` پر عمل درآمد کرتا ہے ، تیار کرتا ہے:

- `build/checksums.sha256` - SHA256 `sha256sum -c` کے لئے موزوں ظاہر۔
- `build/release.json` - میٹا ڈیٹا (`tag` ، `generated_at` ، `source`) ہر کار/مینی فیسٹ میں سیٹ کریں۔

دونوں فائلوں کو کار خلاصہ کے ساتھ فائل کریں تاکہ جائزہ لینے والے نمونے کا موازنہ کرسکیں
غیر ساختہ پیش نظارہ۔

## مرحلہ 2 - جامد اثاثوں کو پیکج کریںDocusaurus کی آؤٹ پٹ ڈائرکٹری کے خلاف کار پیکجر چلائیں۔ ذیل میں مثال
`artifacts/devportal/` کے تحت تمام نمونے لکھتا ہے۔

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

JSON ڈائجسٹ نے حصہ گنتی ، ڈائجسٹس ، اور پروف پلاننگ اشارے پر قبضہ کیا ہے
`manifest build` اور CI ڈیش بورڈز نے بعد میں دوبارہ استعمال کیا۔

## مرحلہ 2B - پیکیج ساتھی OpenAPI اور SBOM

DOCS-7 کے لئے پورٹل سائٹ ، اسنیپ شاٹ OpenAPI ، اور SBOM پے لوڈ کی اشاعت کی ضرورت ہے
جیسا کہ الگ الگ ظاہر ہوتا ہے تاکہ گیٹ وے ہیڈر کو ایک ساتھ رکھ سکے
`Sora-Proof`/`Sora-Content-CID` ہر آلے کے لئے۔ رہائی مددگار
(`scripts/sorafs-pin-release.sh`) پہلے ہی پیکیجز ڈائرکٹری OpenAPI
(`static/openapi/`) اور SBOMs الگ کاروں میں `syft` کے ذریعے جاری کیا گیا
`openapi.*`/`*-sbom.*` اور میٹا ڈیٹا کو ریکارڈ کرتا ہے
`artifacts/sorafs/portal.additional_assets.json`۔ جب دستی بہاؤ چلاتے ہو ،
ہر پے لوڈ کے لئے اپنے سابقہ ​​اور میٹا ڈیٹا ٹیگ کے ساتھ 2-4 اقدامات دہرائیں
(مثال کے طور پر `--car-out "$OUT"/openapi.car` مزید
`--metadata alias_label=docs.sora.link/openapi`)۔ ہر ظاہر/عرف جوڑی کو ریکارڈ کریں
DNS کو تبدیل کرنے سے پہلے Torii (سائٹ ، OpenAPI ، پورٹل SBOM ، OpenAPI SBOM) پر
گیٹ وے تمام شائع شدہ نمونے کے لئے اسٹپلڈ ٹیسٹ پیش کرسکتا ہے۔

## مرحلہ 3 - منشور بنائیں

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

اپنی ریلیز ونڈو کے مطابق پن پالیسی کے جھنڈے مرتب کریں (مثال کے طور پر ، `-اسٹوریج کلاس
کینریوں کے لئے گرم)۔ JSON مختلف حالت اختیاری ہے لیکن کوڈ کے جائزے کے لئے آسان ہے۔

## مرحلہ 4 - Sigstore کے ساتھ سائن کریں

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

اس بنڈل میں منشور کے ڈائجسٹ ، گرن ڈائجسٹ اور ٹوکن کا ایک بلیک 3 ہیش ریکارڈ کیا گیا ہے
OIDC JWT کو برقرار رکھے بغیر۔ بنڈل اور علیحدہ دستخط دونوں کو محفوظ کریں۔ کی پروموشنز
پیداوار دوبارہ دستخط کرنے کے بجائے ایک ہی نمونے کو دوبارہ استعمال کرسکتی ہے۔ مقامی پھانسی
آپ فراہم کنندہ کے جھنڈوں کو `--identity-token-env` (یا سیٹ سیٹ سے تبدیل کرسکتے ہیں
`SIGSTORE_ID_TOKEN` ماحول میں) جب بیرونی OIDC مددگار ٹوکن جاری کرتا ہے۔

## مرحلہ 5 - رجسٹری پن پر بھیجیں

Torii پر دستخط شدہ منشور (اور حصہ منصوبہ) بھیجیں۔ ہمیشہ ایک سمری کی درخواست کریں تاکہ
نتیجے میں داخلہ/عرف انتہائی قابل عمل ہے۔

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

جب ایک پیش نظارہ یا کینری عرف (`docs-preview.sora`) تعینات کیا جاتا ہے تو ، دہرائیں
ایک انوکھا عرف کے ساتھ بھیجیں تاکہ QA پروموشن سے پہلے مواد کی تصدیق کرسکے
پیداوار

عرف بائنڈنگ کے لئے تین فیلڈز کی ضرورت ہے: `--alias-namespace` ، `--alias-name` ، اور `--alias-proof`۔
جب درخواست منظور ہوجاتی ہے تو گورننس پروف بنڈل (بیس 64 یا Norito بائٹس) تیار کرتا ہے
عرف کی ؛ اسے CI رازوں میں محفوظ کریں اور `manifest submit` پر کال کرنے سے پہلے اسے فائل کے طور پر بے نقاب کریں۔
جب آپ صرف DNS کو چھوئے بغیر ہی منشور کو مرتب کرنے کا ارادہ رکھتے ہیں تو عرف پرچموں کو غیر سیٹ چھوڑ دیں۔

## مرحلہ 5B - گورننس کی تجویز تیار کریں

ہر منشور کو پارلیمنٹ کے لئے تیار تجویز کے ساتھ سفر کرنا ہوگا تاکہ کوئی بھی شہری
سورہ مراعات یافتہ اسناد طلب کیے بغیر تبدیلی کو متعارف کراسکتی ہے۔ قدموں کے بعد
جمع کروائیں/دستخط کریں ، عملدرآمد کریں:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
````portal.pin.proposal.json` نے کیننیکل انسٹرکشن `RegisterPinManifest` کو اپنی گرفت میں لے لیا ،
حصہ ڈائجسٹ ، پالیسی اور عرف ٹریک۔ اسے گورننس کے ٹکٹ سے یا اس سے منسلک کریں
پارلیمنٹ پورٹل تاکہ مندوبین نمونے کی تعمیر نو کے بغیر پے لوڈ کا موازنہ کرسکیں۔
چونکہ کمانڈ کبھی بھی Torii کی اتھارٹی کی کلید کو نہیں چھوتی ہے ، لہذا کوئی بھی شہری تحریر کرسکتا ہے
مقامی طور پر تجویز کیا گیا۔

## مرحلہ 6 - ٹیسٹ اور ٹیلی میٹری کی تصدیق کریں

فکسنگ کے بعد ، عزم کی توثیق کے اقدامات پر عمل کریں:

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

- `torii_sorafs_gateway_refusals_total` اور چیک کریں
  `torii_sorafs_replication_sla_total{outcome="missed"}` بے ضابطگیوں کے لئے۔
- `npm run probe:portal` کو TRY-IT پراکسی اور رجسٹرڈ لنکس استعمال کرنے کے لئے چلائیں
  نئے پوسٹ کردہ مواد کے خلاف۔
- بیان کردہ نگرانی کے شواہد کو اپنی گرفت میں لے لیتا ہے
  [پبلشنگ اینڈ مانیٹرنگ] (./publishing-monitoring.md) دستاویزات -3 سی مشاہدہ گیٹ کے لئے
  اشاعت کے اقدامات کے ساتھ ساتھ مطمئن ہے۔ مددگار اب متعدد آدانوں کو قبول کرتا ہے
  `bindings` (سائٹ ، OpenAPI ، پورٹل SBOM ، OpenAPI SBOM) اور `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` کا اطلاق کریں
  اختیاری گارڈ `hostname` کے ذریعے ہدف کے میزبان پر۔ ذیل میں درخواست دونوں لکھتی ہے
  ثبوت کے بنڈل کے طور پر سنگل JSON خلاصہ (`portal.json` ، `tryit.json` ، `binding.json` اور
  `checksums.sha256`) ریلیز ڈائرکٹری کے تحت:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## مرحلہ 6A - گیٹ وے کے سرٹیفکیٹ کی منصوبہ بندی کریں

گار پیکٹ بنانے سے پہلے TLS SAN/چیلنج پلان حاصل کریں تاکہ گیٹ وے کمپیوٹر اور
ڈی این ایس کے منظوری ایک ہی ثبوت کا جائزہ لیتے ہیں۔ نیا مددگار آٹومیشن آدانوں کی عکاسی کرتا ہے
ڈی جی 3 لسٹنگ کینونیکل وائلڈ کارڈ کے میزبان ، خوبصورت میزبان سینز ، DNS-01 ٹیگ ، اور تجویز کردہ ACME چیلنجز:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

ریلیز کے بنڈل کے ساتھ JSON کا ارتکاب کریں (یا اسے تبدیلی کے ٹکٹ کے ساتھ اپ لوڈ کریں) تاکہ آپریٹرز
`torii.sorafs_gateway.acme` Torii اور جائزہ لینے والوں کی تشکیل میں SAN اقدار کو چسپاں کرسکتے ہیں
GAR کا میزبان مشتق دوبارہ چلائے بغیر کیننیکل/خوبصورت نقشہ سازی کی تصدیق کرسکتا ہے۔ شامل کریں
اضافی `--name` ہر ایک کے لئے ایک ہی ریلیز میں لاحقہ لاحقہ کے لئے دلائل۔

## مرحلہ 6 بی - کیننیکل میزبان میپنگز اخذ کریں

GAR پے لوڈ کو ٹیون کرنے سے پہلے ، ہر عرف کے لئے ڈٹرمینسٹک میزبان نقشہ سازی کو ریکارڈ کریں۔
`cargo xtask soradns-hosts` ہر `--name` اس کے کیننیکل ٹیگ میں ہیش کرتا ہے
(`<base32>.gw.sora.id`) ، مطلوبہ وائلڈ کارڈ (`*.gw.sora.id`) جاری کریں اور خوبصورت میزبان اخذ کریں
(`<alias>.gw.sora.name`)۔ ریلیز نمونے میں آؤٹ پٹ کو برقرار رکھیں تاکہ جائزہ لینے والے کر سکیں
DG-3 میپنگ کا موازنہ GAR شپمنٹ کے ساتھ کرسکتا ہے:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

جب گار یا گیٹ وے بائنڈنگ سے تعلق رکھنے والا JSON تیزی سے ناکام ہونے کے لئے `--verify-host-patterns <file>` استعمال کریں
مطلوبہ میزبانوں میں سے ایک کو چھوڑ دیتا ہے۔ مددگار متعدد توثیق فائلوں کو قبول کرتا ہے
گار ٹیمپلیٹ اور `portal.gateway.binding.json` دونوں کا آسان لنٹ اسی دعوت نامے میں شامل ہے:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```JSON کا خلاصہ اور تصدیق لاگ ان DNS/گیٹ وے تبدیلی کے ٹکٹ پر منسلک کریں تاکہ
آڈیٹر اسکرپٹ کو دوبارہ چلائے بغیر کیننیکل ، وائلڈ کارڈ اور خوبصورت میزبانوں کی تصدیق کرسکتے ہیں۔
کمانڈ کو دوبارہ عمل کریں جب بنڈل میں نئے عرفات کو شامل کیا جائے تاکہ گار تازہ کاری کریں
ایک ہی ثبوت کے وارث.

## مرحلہ 7 - DNS کٹ اوور ڈسکرپٹر تیار کریں

پروڈکشن کٹ اوور کے لئے ایک آڈٹیبل چینج پیکیج کی ضرورت ہوتی ہے۔ ایک کامیاب جمع کرانے کے بعد
(عرف بائنڈنگ) ، مددگار مسائل
`artifacts/sorafs/portal.dns-cutover.json` ، گرفتاری:

- عرف بائنڈنگ میٹا ڈیٹا (نام کی جگہ/نام/ثبوت ، منشور ڈائجسٹ ، یو آر ایل Torii ،
  عہد بھیجا ، اتھارٹی) ؛
- ریلیز سیاق و سباق (ٹیگ ، عرف لیبل ، منشور/کار کے راستے ، چنک پلان ، بنڈل Sigstore) ؛
- توثیق کے اشارے (تحقیقات کمانڈ ، عرف + اختتامی نقطہ Torii) ؛ اور
- اختیاری تبدیلی پر قابو پانے والے فیلڈز (ٹکٹ ID ، کٹ اوور ونڈو ، OPS رابطہ ،
  میزبان نام/پروڈکشن زون) ؛
- روٹ پروموشن میٹا ڈیٹا ہیڈر `Sora-Route-Binding` سے ماخوذ ہے
  .
  گار اور فال بیک مشقیں ایک ہی ثبوت کا حوالہ دیتی ہیں۔
- تیار کردہ روٹ پلانٹ نمونے (`gateway.route_plan.json` ،
  ہیڈر ٹیمپلیٹس اور اختیاری رول بیک ہیڈر) تاکہ ٹکٹ اور رول بیک ہکس کو تبدیل کریں
  سی آئی سے لنٹ اس بات کی تصدیق کرسکتا ہے کہ ہر ڈی جی 3 پیکیج پروموشن/رول بیک منصوبوں کا حوالہ دیتا ہے
  منظوری سے پہلے کیننیکل ؛
- اختیاری کیشے باطل میٹا ڈیٹا (پرج اختتامی نقطہ ، آتھ متغیر ، JSON پے لوڈ ،
  اور کمانڈ مثال `curl`) ؛ اور
- رول بیک کے اشارے پچھلے ڈسکرپٹر کی طرف اشارہ کرتے ہیں (ریلیز ٹیگ اور مینی فیسٹ ڈائجسٹ)
  تاکہ ٹکٹ ایک عین مطابق فال بیک بیک راستہ پر قبضہ کریں۔

جب رہائی میں کیشے صاف کرنے کی ضرورت ہوتی ہے تو ، یہ کٹ اوور ڈسکرپٹر کے ساتھ ہی ایک کیننیکل پلان تیار کرتا ہے:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

نتیجے میں `portal.cache_plan.json` کو DG-3 پیکیج سے جوڑتا ہے تاکہ آپریٹرز کرسکیں
جب `PURGE` درخواستیں جاری کرتے ہو تو ڈٹرمینسٹک میزبان/راستے (اور مصنف کے اشارے سے ملتے ہیں)۔
ڈسکرپٹر کا اختیاری کیشے سیکشن اس فائل کا براہ راست حوالہ دے سکتا ہے ،
تبدیلی پر قابو پانے والے جائزہ لینے والوں کو بالکل ٹھیک رکھنا ہے کہ کس اختتامی نکات کو صاف کیا جاتا ہے
ایک کٹور کے دوران

ہر DG-3 پیکیج کو بھی پروموشن + رول بیک چیک لسٹ کی ضرورت ہوتی ہے۔ جنرل کے ذریعے
`cargo xtask soradns-route-plan` لہذا تبدیلی پر قابو پانے والے جائزہ لینے والے اقدامات پر عمل کرسکتے ہیں
عرفی پریف لائٹ ، کٹ اوور اور رول بیک برائے عرف:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

جاری کردہ `gateway.route_plan.json` نے کیننیکل/خوبصورت میزبانوں ، صحت کی جانچ پڑتال کی یاد دہانیوں کی گرفت کی
مراحل میں ، گار بائنڈنگ اپڈیٹس ، کیشے صاف اور رول بیک ایکشنز۔ اس کے ساتھ شامل کریں
تبدیلی کا ٹکٹ جمع کروانے سے پہلے گار/بائنڈنگ/کٹ اوور نمونے
اسکرپٹ کے ساتھ انہی اقدامات کو منظور کریں۔

`scripts/generate-dns-cutover-plan.mjs` اس ڈسکرپٹر کو چلاتا ہے اور خود بخود چلتا ہے
`sorafs-pin-release.sh`۔ اسے دستی طور پر دوبارہ تخلیق کرنا یا اپنی مرضی کے مطابق بنانا:```bash
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

پن ہیلپر چلانے سے پہلے ماحولیاتی متغیر کے ذریعہ اختیاری میٹا ڈیٹا کو پُر کریں:

| متغیر | مقصد |
| --- | --- |
| `DNS_CHANGE_TICKET` | ڈسکرپٹر میں ٹکٹ کی شناخت محفوظ ہے۔ |
| `DNS_CUTOVER_WINDOW` | ISO8601 کٹ اوور ونڈو (جیسے `2026-03-21T15:00Z/2026-03-21T15:30Z`)۔ |
| `DNS_HOSTNAME` ، `DNS_ZONE` | پروڈکشن میزبان نام + مستند زون۔ |
| `DNS_OPS_CONTACT` | آن کال عرف یا بڑھتی ہوئی رابطہ۔ |
| `DNS_CACHE_PURGE_ENDPOINT` | ڈسکرپٹر میں رجسٹرڈ کیشے صاف کریں۔ |
| `DNS_CACHE_PURGE_AUTH_ENV` | پرج ٹوکن (پہلے سے طے شدہ: `CACHE_PURGE_TOKEN`) پر مشتمل var var۔ |
| `DNS_PREVIOUS_PLAN` | رول بیک میٹا ڈیٹا کے لئے پچھلے کٹ اوور ڈسکرپٹر کا راستہ۔ |

JSON کو DNS تبدیلی کے جائزے سے منسلک کریں تاکہ منظوری دینے والے ظاہر ہضموں کی تصدیق کرسکیں ،
عرف بائنڈنگ اور تحقیقات کے احکامات CI لاگز کا جائزہ لینے کے بغیر۔ سی ایل آئی جھنڈے
`--dns-change-ticket` ، `--dns-cutover-window` ، `--dns-hostname` ،
`--dns-zone` ، `--ops-contact` ، `--cache-purge-endpoint` ،
`--cache-purge-auth-env` ، اور `--previous-dns-plan` وہی اوور رائڈس فراہم کرتا ہے
جب مددگار کو CI کے باہر پھانسی دی جاتی ہے۔

## مرحلہ 8 - حل کرنے والے زون فائل کنکال (اختیاری)

جب پروڈکشن کٹ اوور ونڈو معلوم ہوجائے تو ، ریلیز اسکرپٹ جاری کرسکتا ہے
خود بخود حل کرنے کے لئے ایس این ایس زون فائل کنکال اور ٹکڑا۔ مطلوبہ DNS ریکارڈ پاس کریں
اور ماحولیاتی متغیرات یا سی ایل آئی کے اختیارات کے ذریعے میٹا ڈیٹا ؛ مددگار کال کرے گا
`scripts/sns_zonefile_skeleton.py` کٹ اوور ڈسکرپٹر تیار کرنے کے فورا بعد۔
کم از کم ایک A/AAAA/CNAME ویلیو اور GAR ڈائجسٹ (دستخط شدہ GAR پے لوڈ کا BLAK3-256) فراہم کریں۔ اگر
زون/میزبان نام جانا جاتا ہے اور `--dns-zonefile-out` کو نظرانداز کیا جاتا ہے ، مددگار لکھتا ہے
`artifacts/sns/zonefiles/<zone>/<hostname>.json` اور بھریں
`ops/soradns/static_zones.<hostname>.json` حل کرنے کے لئے ٹکڑا کے طور پر۔| متغیر/پرچم | مقصد |
| --- | --- |
| `DNS_ZONEFILE_OUT` ، `--dns-zonefile-out` | پیدا شدہ زون فائل کنکال کا راستہ۔ |
| `DNS_ZONEFILE_RESOLVER_SNIPPET` ، `--dns-zonefile-resolver-snippet` | اسنیپٹ پاتھ کو حل کریں (پہلے سے طے شدہ: `ops/soradns/static_zones.<hostname>.json` جب چھوڑ دیا جاتا ہے)۔ |
| `DNS_ZONEFILE_TTL` ، `--dns-zonefile-ttl` | ٹی ٹی ایل نے تیار کردہ ریکارڈوں پر لاگو کیا (پہلے سے طے شدہ: 600 سیکنڈ)۔ |
| `DNS_ZONEFILE_IPV4` ، `--dns-zonefile-ipv4` | IPv4 پتے (کوما سے الگ الگ ENV یا تکرار کرنے والا CLI پرچم)۔ |
| `DNS_ZONEFILE_IPV6` ، `--dns-zonefile-ipv6` | IPv6 پتے۔ |
| `DNS_ZONEFILE_CNAME` ، `--dns-zonefile-cname` | ہدف CNAME اختیاری۔ |
| `DNS_ZONEFILE_SPKI` ، `--dns-zonefile-spki-pin` | SPKI SHA-256 (BASE64) پن۔ |
| `DNS_ZONEFILE_TXT` ، `--dns-zonefile-txt` | اضافی TXT اندراجات (`key=value`)۔ |
| `DNS_ZONEFILE_VERSION` ، `--dns-zonefile-version` | کمپیوٹڈ زون فائل ورژن لیبل کا اوور رائڈ۔ |
| `DNS_ZONEFILE_EFFECTIVE_AT` ، `--dns-zonefile-effective-at` | کٹ اوور ونڈو شروع کرنے کے بجائے ٹائم اسٹیمپ `effective_at` (RFC3339)۔ |
| `DNS_ZONEFILE_PROOF` ، `--dns-zonefile-proof` | میٹا ڈیٹا میں رجسٹرڈ لفظی ثبوت کی حد سے تجاوز کریں۔ |
| `DNS_ZONEFILE_CID` ، `--dns-zonefile-cid` | میٹا ڈیٹا میں رجسٹرڈ سی آئی ڈی کا اوور رائڈ۔ |
| `DNS_ZONEFILE_FREEZE_STATE` ، `--dns-zonefile-freeze-state` | گارڈین منجمد کی حیثیت (نرم ، سخت ، پگھلنا ، نگرانی ، ہنگامی صورتحال)۔ |
| `DNS_ZONEFILE_FREEZE_TICKET` ، `--dns-zonefile-freeze-ticket` | گارڈین/کونسل کے ٹکٹ کا حوالہ منجمد کرنے کے لئے۔ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT` ، `--dns-zonefile-freeze-expires-at` | پگھلنے کے لئے RFC3339 ٹائم اسٹیمپ۔ |
| `DNS_ZONEFILE_FREEZE_NOTES` ، `--dns-zonefile-freeze-note` | اضافی منجمد نوٹ (کوما سے الگ الگ env یا تکرار کرنے والا جھنڈا)۔ |
| `DNS_GAR_DIGEST` ، `--dns-gar-digest` | دستخط شدہ GAR پے لوڈ کے بلیک 3-256 (ہیکس) کو ہضم کریں۔ جب گیٹ وے بائنڈنگز ہوں تو ضروری ہے۔ |

گٹ ہب ایکشن ورک فلو ان اقدار کو ذخیرہ راز سے پڑھتا ہے تاکہ ہر پن کا ہر پن
پیداوار زون فائل نمونے کو خود بخود خارج کرتی ہے۔ مندرجہ ذیل راز مرتب کریں
(تاروں میں ملٹی ویلیو فیلڈز کے لئے کوما سے الگ الگ فہرستیں شامل ہوسکتی ہیں):

| خفیہ | مقصد |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME` ، `DOCS_SORAFS_DNS_ZONE` | میزبان نام/پروڈکشن زون مددگار کو منتقل ہوا۔ |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | آن کال عرفیئس ڈسکرپٹر میں محفوظ ہے۔ |
| `DOCS_SORAFS_ZONEFILE_IPV4` ، `DOCS_SORAFS_ZONEFILE_IPV6` | IPv4/IPv6 ریکارڈ شائع کرنے کے لئے۔ |
| `DOCS_SORAFS_ZONEFILE_CNAME` | ہدف CNAME اختیاری۔ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | ایس پی کے آئی بیس 64 پنوں۔ |
| `DOCS_SORAFS_ZONEFILE_TXT` | اضافی TXT اندراجات۔ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | کنکال میں ریکارڈ شدہ میٹا ڈیٹا منجمد کریں۔ |
| `DOCS_SORAFS_GAR_DIGEST` | دستخط شدہ گار پے لوڈ ہیکس میں بلیک 3 کو ہضم کریں۔ |

`.github/workflows/docs-portal-sorafs-pin.yml` کو متحرک کرکے ، یہ آدانوں کو فراہم کرتا ہے
`dns_change_ticket` اور `dns_cutover_window` ونڈو کے وارث ہونے کے لئے وضاحتی/زون فائل کے لئے
درست. جب خشک رنز پر عملدرآمد کرتے ہو تو انہیں خالی چھوڑ دیں۔

عام درخواست (SN-7 مالک رن بک سے مماثل ہے):

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

مددگار خود بخود تبدیلی کے ٹکٹ کو TXT ان پٹ کے طور پر گھسیٹتا ہے اور اس کے آغاز کو وراثت میں ملتا ہے
کٹور ونڈو بطور ٹائم اسٹیمپ `effective_at` جب تک کہ اوور رائڈ نہ ہو۔ بہاؤ کے لئے
مکمل آپریشن ، `docs/source/sorafs_gateway_dns_owner_runbook.md` دیکھیں۔

### عوامی DNS وفد پر نوٹزون فائل کنکال صرف زون کے لئے مستند ریکارڈوں کی وضاحت کرتا ہے۔ یہاں تک کہ
آپ کو اپنے رجسٹرار میں والدین زون کے NS/DS وفد کو تشکیل دینا ہوگا یا
DNS فراہم کنندہ تاکہ عوامی انٹرنیٹ ناموں کو تلاش کرسکے۔

- اپیکس/ٹی ایل ڈی میں کٹ اوور کے ل ، ، عرف/aname (فراہم کنندہ پر منحصر) استعمال کریں یا
  A/AAAA ریکارڈ شائع کرتا ہے جو گیٹ وے کے کسی بھی کاسٹ IPs کی طرف اشارہ کرتا ہے۔
- ذیلی ڈومینز کے لئے ، اخذ کردہ خوبصورت میزبان کو ایک نام شائع کریں
  (`<fqdn>.gw.sora.name`)۔
- کیننیکل میزبان (`<hash>.gw.sora.id`) گیٹ وے ڈومین کے تحت رہتا ہے
  اور یہ آپ کے عوامی علاقے میں شائع نہیں ہوا ہے۔

### گیٹ وے ہیڈر ٹیمپلیٹ

تعینات مددگار `portal.gateway.headers.txt` اور بھی جاری کرتا ہے
`portal.gateway.binding.json` ، دو آلات جو DG-3 کی ضرورت کو پورا کرتے ہیں
گیٹ وے-مشمولات کا پابند:

- `portal.gateway.headers.txt` میں مکمل HTTP ہیڈر بلاک (بشمول شامل ہے
  `Sora-Name` ، `Sora-Content-CID` ، `Sora-Proof` ، CSP ، HSTS ، اور تفصیل کار
  `Sora-Route-Binding`) اس ایج گیٹ ویز کو ہر ردعمل میں رکھنا چاہئے۔
- `portal.gateway.binding.json` مشین سے پڑھنے کے قابل شکل میں ایک ہی معلومات کو ریکارڈ کرتا ہے
  تاکہ ٹکٹوں اور آٹومیشن کو تبدیل کریں بغیر میزبان/سی آئی ڈی بائنڈنگ کا موازنہ کیا جاسکے
  کھرچنا شیل آؤٹ پٹ۔

وہ خود بخود پیدا ہوجاتے ہیں
`cargo xtask soradns-binding-template`
اور عرف پر قبضہ کریں ، منشور ڈائجسٹ اور گیٹ وے کا میزبان نام جو ہے
`sorafs-pin-release.sh` میں تبدیل ہوا۔ ہیڈر بلاک کو دوبارہ تخلیق یا اپنی مرضی کے مطابق بنانا ،
چلائیں:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

اوور رائڈ کے لئے `--csp-template` ، `--permissions-template` ، یا `--hsts-template` پاس کریں
پہلے سے طے شدہ ہیڈر ٹیمپلیٹس میں سے جب کسی تعیناتی کو اضافی ہدایت کی ضرورت ہوتی ہے۔
ہیڈر کو مکمل طور پر ختم کرنے کے لئے ان کو موجودہ `--no-*` سوئچ کے ساتھ جوڑیں۔

سی ڈی این تبدیلی کی درخواست کے ساتھ ہیڈر کے ٹکڑوں کو منسلک کریں اور JSON دستاویز کو کھانا کھلائیں
گیٹ وے آٹومیشن پائپ لائن پر تاکہ اصل میزبان پروموشن سے مماثل ہو
رہائی کا ثبوت۔

ریلیز اسکرپٹ خود بخود توثیق مددگار چلاتا ہے تاکہ
DG-3 ٹکٹوں میں ہمیشہ حالیہ ثبوت شامل ہوتے ہیں۔ اسے دوبارہ دستی طور پر چلائیں
جب آپ JSON کو ہاتھ سے بائنڈنگ میں ترمیم کرتے ہیں:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

کمانڈ اسٹپلڈ `Sora-Proof` پے لوڈ کو ضابطہ کشائی کرتا ہے ، اس بات کو یقینی بناتا ہے کہ `Sora-Route-Binding` میٹا ڈیٹا
منشور + میزبان نام کے CID سے مماثل ہے ، اور اگر کوئی ہیڈر کھڑا ہوتا ہے تو تیزی سے ناکام ہوجاتا ہے۔
جب بھی آپ چلتے ہیں تو اپنے دوسرے تعیناتی نمونے کے ساتھ کنسول آؤٹ پٹ کو محفوظ کریں
CI سے باہر کی کمانڈ تاکہ DG-3 جائزہ نگاروں کے پاس اس بات کا ثبوت ہو کہ پابند ہونے کی توثیق کی گئی ہے
کٹ اوور سے پہلے> ** DNS ڈسکرپٹر انضمام: ** `portal.dns-cutover.json` اب ایک حصے کو سرایت کرتا ہے
> `gateway_binding` ان نمونے کی طرف اشارہ کرتے ہوئے (راستے ، مواد سی آئی ڈی ، پروف اسٹیٹس اور
> ہیڈرز کے لغوی لغوی)
> `gateway.route_plan.json` پلس مین اور رول بیک ہیڈر ٹیمپلیٹس۔ ان پر مشتمل ہے
> ہر DG-3 پر بلاکس ٹکٹ تبدیل کریں تاکہ جائزہ لینے والے اقدار کا موازنہ کرسکیں
> عین مطابق `Sora-Name/Sora-Proof/CSP` اور فروغ/رول بیک منصوبوں کی تصدیق کریں
> بلڈ فائل کو کھولے بغیر ثبوت کے بنڈل سے میچ کریں۔

## مرحلہ 9 - اشاعت مانیٹر چلائیں

روڈ میپ آئٹم ** دستاویزات -3 سی ** کے لئے جاری ثبوت کی ضرورت ہے کہ پورٹل ، کوشش کریں پراکسی ، اور
رہائی کے بعد گیٹ وے پابندیاں صحت مند رہیں۔ مستحکم مانیٹر چلائیں
7-8 قدموں کے فورا بعد اور اسے اپنے پروگرام شدہ تحقیقات سے مربوط کریں:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` کنفگ فائل کو لوڈ کرتا ہے (دیکھیں
  `docs/portal/docs/devportal/publishing-monitoring.md` اسکیمیٹک کے لئے) اور
  تین چیکوں پر عمل کریں: پورٹل پاتھ پروبس + سی ایس پی/اجازت-پالیسی کی توثیق ،
  اس پر پراکسی تحقیقات کی کوشش کریں (اختیاری طور پر آپ کے اختتامی نقطہ `/metrics` کو مارنا) ، اور تصدیق کنندہ
  گیٹ وے بائنڈنگ ماڈیول (`cargo xtask soradns-verify-binding`) جس کے لئے اب موجودگی کی ضرورت ہے اور
  عرف/مینی فیسٹ چیک کے ساتھ سورہ-مشمولات سیڈ کی متوقع قیمت۔
- کمانڈ غیر صفر کے ساتھ ختم ہوجاتی ہے جب CI ، CRON ملازمتوں ، یا آپریٹرز کے لئے کوئی بھی تحقیقات ناکام ہوجاتی ہے
  عرفی ناموں کو فروغ دینے سے پہلے رن بکس ایک ریلیز کو روک سکتے ہیں۔
- پاسنگ `--json-out` ریاست فی ہدف کے ساتھ ایک JSON خلاصہ لکھتا ہے۔ `--evidence-dir`
  `summary.json` ، `portal.json` ، `tryit.json` ، `binding.json` اور `checksums.sha256` تاکہ یہ جاری کریں
  گورننس کے جائزہ لینے والے دوبارہ چلانے والے مانیٹر کے بغیر نتائج کا موازنہ کرسکتے ہیں۔ آرکائیو
  `artifacts/sorafs/<tag>/monitoring/` کے تحت یہ ڈائرکٹری بنڈل Sigstore اور کے ساتھ مل کر
  DNS کٹ اوور ڈسریکٹر۔
- مانیٹر آؤٹ پٹ ، Grafana (`dashboards/grafana/docs_portal.json`) کی برآمد پر مشتمل ہے ،
  اور ریلیز ٹکٹ پر الرٹ مینجر ڈرل ID تاکہ DOCS-3C SLO ہوسکے
  بعد میں آڈٹ. سرشار پبلشنگ مانیٹر پلے بک میں رہتا ہے
  `docs/portal/docs/devportal/publishing-monitoring.md`۔

پورٹل تحقیقات کو HTTPS کی ضرورت ہوتی ہے اور بیس URLs کو مسترد کرنا `http://` جب تک `allowInsecureHttp`
یہ مانیٹر ترتیب میں تشکیل دیا گیا ہے۔ پیداوار/اسٹیجنگ اہداف کو TLS اور صرف میں رکھیں
مقامی پیش نظاروں کے لئے اوور رائڈ کو قابل بنائیں۔

ایک بار پورٹل ایک بار بلڈکائٹ/کرون میں `npm run monitor:publishing` کے ذریعے مانیٹر کو خودکار کریں
زندہ ہے۔ اسی کمانڈ ، جو پیداوار کے یو آر ایل کی طرف اشارہ کرتے ہیں ، صحت کی جانچ پڑتال کرتے ہیں
مسلسل جو SRE/دستاویزات ریلیز کے مابین استعمال کرتے ہیں۔

## `sorafs-pin-release.sh` کے ساتھ آٹومیشن

`docs/portal/scripts/sorafs-pin-release.sh` اقدامات 2-6 سے منسلک کرتا ہے۔ یہ:1. فائل `build/` ایک ڈٹرمینسٹک ٹربال میں ،
2. چلائیں `car pack` ، `manifest build` ، `manifest sign` ، `manifest verify-signature` ،
   اور `proof verify` ،
3. اختیاری طور پر `manifest submit` چلائیں (بشمول عرف بائنڈنگ) جب اسناد موجود ہوں
   Torii ، اور
4. `artifacts/sorafs/portal.pin.report.json` لکھیں ، اختیاری
  `portal.pin.proposal.json` ، DNS کٹ اوور ڈسکرپٹر (گذارشات کے بعد) ،
  اور گیٹ وے بائنڈنگ بنڈل (`portal.gateway.binding.json` پلس ہیڈر بلاک)
  تاکہ گورننس ، نیٹ ورکنگ اور او پی ایس ٹیمیں ثبوت کے بنڈل کا موازنہ کرسکیں
  CI لاگز کا جائزہ لینے کے بغیر۔

`PIN_ALIAS` ، `PIN_ALIAS_NAMESPACE` ، `PIN_ALIAS_NAME` ، اور (اختیاری طور پر) تشکیل دیتا ہے
اسکرپٹ کی درخواست کرنے سے پہلے `PIN_ALIAS_PROOF_PATH`۔ خشک کرنے کے لئے `--skip-submit` استعمال کریں
رنز ؛ ذیل میں بیان کردہ گٹ ہب ورک فلو انٹری `perform_submit` کے ذریعے اس کو ٹوگل کرتا ہے۔

## مرحلہ 8 - OpenAPI نردجیکرن اور SBOM پیکیج شائع کریں

دستاویزات -7 کا تقاضا ہے کہ پورٹل بلڈ ، OpenAPI تفصیلات اور SBOM نمونے کا سفر
اسی عزم پائپ لائن کے ذریعے۔ موجودہ مددگار تینوں کا احاطہ کرتے ہیں:

1. ** تصریح کو دوبارہ تخلیق کریں اور اس پر دستخط کریں۔ **

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   جب آپ اسنیپ شاٹ کو محفوظ رکھنا چاہتے ہیں تو `--version=<label>` کے ذریعے ریلیز ٹیگ پاس کریں
   تاریخ (مثال کے طور پر `2025-q3`)۔ مددگار اسنیپ شاٹ لکھتا ہے
   `static/openapi/versions/<label>/torii.json` ، اس میں آئینہ دار ہے
   `versions/current` ، اور میٹا ڈیٹا کو ریکارڈ کرتا ہے (SHA-256 ، ظاہر کی حیثیت ، اور
   `static/openapi/versions.json` میں تازہ ترین ٹائم اسٹیمپ)۔ ڈویلپر پورٹل
   اس انڈیکس کو پڑھتا ہے تاکہ سویگر/ریپڈوک پینل ایک ورژن سلیکٹر پیش کرسکیں
   اور متعلقہ ڈائجسٹ/دستخط آن لائن ڈسپلے کریں۔ `--version` کو چھوڑیں
   پچھلی ریلیز برقرار ہے اور صرف `current` + `latest` پوائنٹرز کو تازہ دم کرتی ہے۔

   مینی فیسٹ نے SHA-256/BLAKE3 ڈائجسٹز کو اپنی گرفت میں لے لیا تاکہ گیٹ وے اسٹپل ہو سکے
   `/reference/torii-swagger` کے لئے ہیڈر `Sora-Proof`۔

2.
   `docs/source/sorafs_release_pipeline_plan.md` کے مطابق۔ باہر نکلیں
   تعمیراتی نمونے کے آگے:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. ** ہر پے لوڈ کو کار میں پیکج کریں۔ **

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

   مرکزی سائٹ کے طور پر `manifest build` / `manifest sign` کے انہی اقدامات پر عمل کریں ،
   فی نمونہ (مثال کے طور پر ، `docs-openapi.sora` تفصیلات کے ل and اور
   دستخط شدہ SBOM بنڈل کیلئے `docs-sbom.sora`)۔ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ
   گارس اور رول بیک ٹکٹ عین مطابق پے لوڈ تک محدود ہیں۔

4. ** جمع کروائیں اور پابند ہوں۔ ** موجودہ اتھارٹی اور Sigstore بنڈل کو دوبارہ استعمال کریں ، لیکن رجسٹر ہوں
   عرف ریلیز چیک لسٹ میں ٹپلس تاکہ آڈیٹرز ٹریک کرسکیں کہ سورہ کا نام کس نام ہے
   نقشے جس میں ظاہر ہضم ہوتا ہے۔

پورٹل بلڈ کے ساتھ اسپیک/ایس بی او ایم کا انکشاف کرنا یقینی بناتا ہے کہ ہر ایک کو یقینی بناتا ہے
ریلیز میں پیکر کو دوبارہ چلائے بغیر نمونے کا مکمل سیٹ شامل ہے۔

### آٹومیشن ہیلپر (CI/پیکیج اسکرپٹ)`./ci/package_docs_portal_sorafs.sh` 1-8 کے قدموں کو انکوڈ کرتا ہے تاکہ روڈ میپ آئٹم
** دستاویزات -7 ** کو ایک ہی کمانڈ کے ساتھ استعمال کیا جاسکتا ہے۔ مددگار:

- پورٹل (`npm ci` ، SYNC OpenAPI/نوریٹو ، ویجیٹ ٹیسٹ) کی مطلوبہ تیاری پر عمل کریں۔
- `sorafs_cli` کے ذریعے پورٹل کے کاروں اور ظاہر جوڑے ، OpenAPI اور SBOM جاری کرتے ہیں۔
- اختیاری طور پر `sorafs_cli proof verify` (`--proof`) اور اشارے Sigstore چلاتا ہے
  (`--sign` ، `--sigstore-provider` ، `--sigstore-audience`) ؛
- `artifacts/devportal/sorafs/<timestamp>/` کے تحت تمام نمونے چھوڑیں
  `package_summary.json` لکھیں تاکہ CI/ریلیز ٹولنگ بنڈل کو کھا سکے۔ اور
- تازہ ترین رن کی طرف اشارہ کرنے کے لئے `artifacts/devportal/sorafs/latest` کو تازہ کریں۔

مثال (Sigstore + POR کے ساتھ مکمل پائپ لائن):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

جاننے کے لئے جھنڈے:

- `--out <dir>` - نمونے کی جڑ اوور رائڈ (ڈیفالٹ ٹائم اسٹیمپ کے ساتھ فولڈرز کو برقرار رکھتا ہے)۔
- `--skip-build` - موجودہ `docs/portal/build` (جب CI نہیں کرسکتا ہے تو مفید ہے
  آف لائن آئینے کے ذریعہ دوبارہ تعمیر)۔
- `--skip-sync-openapi` - `npm run sync-openapi` جب `cargo xtask openapi` کو چھوڑ دیتا ہے
  کریٹس تک نہیں پہنچ سکتے۔
- `--skip-sbom` - جب بائنری انسٹال نہیں ہوتا ہے تو `syft` کو فون کرنے سے پرہیز کریں (اسکرپٹ پرنٹس
  اس کے بجائے ایک انتباہ)۔
- `--proof` - ہر کار/منشور جوڑی کے لئے `sorafs_cli proof verify` چلتا ہے۔ کے پے لوڈ
  متعدد فائلوں کو ابھی بھی CLI میں chunk-plan کی مدد کی ضرورت ہے ، لہذا اس جھنڈے کو چھوڑ دیں
  اگر آپ کو `plan chunk count` غلطیوں کا سامنا کرنا پڑتا ہے تو غیر سیٹ کریں اور جب دستی طور پر چیک کریں
  اپ اسٹریم گیٹ پہنچتا ہے۔
- `--sign` - `sorafs_cli manifest sign` کی درخواست کرتا ہے۔ کے ساتھ ایک ٹوکن فراہم کرتا ہے
  `SIGSTORE_ID_TOKEN` (یا `--sigstore-token-env`) یا سی ایل آئی کو استعمال کرنے دیں
  `--sigstore-provider/--sigstore-audience`۔

جب پروڈکشن نمونے بھیجتے ہو تو `docs/portal/scripts/sorafs-pin-release.sh` استعمال کریں۔
اب پورٹل ، OpenAPI اور SBOM پے لوڈ کو پیک کریں ، ہر مینی فیسٹ پر دستخط کریں ، اور میٹا ڈیٹا ریکارڈ کریں
`portal.additional_assets.json` میں اضافی اثاثے۔ مددگار ایک ہی اختیاری نوبس کو سمجھتا ہے
آئی سی پیکجر کے علاوہ نئے سوئچز `--openapi-*` ، `--portal-sbom-*` ، اور استعمال کیا جاتا ہے
`--openapi-sbom-*` فی نمونہ پر عرف ٹپلس تفویض کرنے کے لئے ، SBOM ماخذ کو اوور رائڈ کریں
`--openapi-sbom-source` ، کچھ پے لوڈ کو چھوڑیں (`--skip-openapi`/`--skip-sbom`) ،
اور `--syft-bin` کے ساتھ غیر ڈیفالٹ بائنری `syft` کی طرف اشارہ کریں۔

اسکرپٹ ہر کمانڈ کو بے نقاب کرتا ہے جس پر عمل درآمد ہوتا ہے۔ لاگ کو ریلیز ٹکٹ پر کاپی کریں
`package_summary.json` کے آگے تاکہ جائزہ لینے والے کار ڈائجسٹ کا موازنہ کرسکیں ،
ایڈہاک شیل آؤٹ پٹ کی جانچ کیے بغیر بنڈل Sigstore کے منصوبے ، اور ہیش۔

## مرحلہ 9 - گیٹ وے کی توثیق + soradns

کٹ اوور کا اعلان کرنے سے پہلے ، جانچ کریں کہ نیا عرف سوردنس کے ذریعہ حل کرتا ہے اور گیٹ وے
بنیادی ثبوت:

1. ** تحقیقات کے گیٹ پر عمل کریں۔ ** `ci/check_sorafs_gateway_probe.sh` ورزش
   `cargo xtask sorafs-gateway-probe` میں ڈیمو فکسچر کے خلاف
   `fixtures/sorafs_gateway/probe_demo/`۔ حقیقی تعیناتیوں کے لئے ، ہدف کے میزبان نام پر تحقیقات کی نشاندہی کریں:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```تحقیقات `Sora-Name` ، `Sora-Proof` ، اور `Sora-Proof-Status` کے مطابق
   `docs/source/sorafs_alias_policy.md` اور مینی فیسٹ کو ہضم کرتے وقت ناکام ہوجاتا ہے ،
   ٹی ٹی ایل یا گار بائنڈنگ منحرف ہیں۔

   ہلکا پھلکا اسپاٹ چیک کے لئے (مثال کے طور پر ، جب صرف پابند بنڈل
   تبدیل شدہ) ، `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>` چلائیں۔
   مددگار پکڑے گئے بائنڈنگ بنڈل کی توثیق کرتا ہے اور رہائی کے لئے آسان ہے
   وہ ٹکٹ جن کو صرف مکمل تحقیقات کی مشق کی بجائے پابند تصدیق کی ضرورت ہوتی ہے۔

2. ** ڈرل کے ثبوت پر قبضہ کریں۔
   `اسکرپٹ/ٹیلی میٹری/رن_سورافس_گیٹ وے_پروبی.ش -سکینریو کے ساتھ تحقیقات
   devuporal-rollout-... `. ریپر کے تحت ہیڈر/لاگز کو بچاتا ہے
   `artifacts/sorafs_gateway_probe/<stamp>/` ، اپ ڈیٹ `ops/drill-log.md` ، اور
   (اختیاری طور پر) فائر رول بیک ہکس یا پیجریڈی پے لوڈ۔ سیٹ
   `--host docs.sora` IP کو سخت کوڈنگ کے بجائے سورادنس روٹ کی توثیق کرنے کے لئے۔

3. ** ڈی این ایس پابندوں کی تصدیق کرتا ہے۔
   تحقیقات (`--gar`) کے ذریعہ حوالہ دیا گیا ہے اور اسے ریلیز شواہد سے جوڑیں۔ حل کرنے کے مالکان
   `tools/soradns-resolver` کے ذریعے اسی ان پٹ کو آئینہ کر سکتے ہیں تاکہ یہ یقینی بنایا جاسکے کہ کیشڈ اندراجات
   نئے منشور کا احترام کریں۔ JSON منسلک کرنے سے پہلے ، چلائیں
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   تو یہ کہ ڈٹرمینسٹک میزبان نقشہ سازی ، مینی فیسٹ میٹا ڈیٹا ، اور ٹیلی میٹری ٹیگز ہیں
   آف لائن کی توثیق کریں۔ مددگار دستخط شدہ گار کے ساتھ ساتھ ایک سمری `--json-out` جاری کرسکتا ہے تاکہ
   جائزہ لینے والوں کے پاس بائنری کھولے بغیر تصدیق کے ثبوت موجود ہیں۔
  نیا گار لکھتے وقت ، ترجیح دیں
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (`--manifest-cid <cid>` پر صرف اس وقت واپس آجاتا ہے جب مینی فیسٹ فائل دستیاب نہ ہو)۔ مددگار
  اب سی آئی ڈی ** اور ** بلیک 3 ڈائجسٹ کو براہ راست JSON منشور ، ٹرمس وائٹ اسپیس سے حاصل کرتا ہے ،
  دہرائے ہوئے `--telemetry-label` جھنڈوں کو کٹوتی کرتا ہے ، لیبلوں کو ترتیب دیتا ہے ، اور پہلے سے طے شدہ ٹیمپلیٹس کو آؤٹ پٹ کرتا ہے
  JSON لکھنے سے پہلے CSP/HSTS/اجازت پالیسی سے
  یہاں تک کہ جب آپریٹرز مختلف گولوں سے لیبل حاصل کرتے ہیں۔

4. ** عرفی میٹرکس کا مشاہدہ کریں۔ ** `torii_sorafs_alias_cache_refresh_duration_ms` کو برقرار رکھیں
   اور `torii_sorafs_gateway_refusals_total{profile="docs"}` اسکرین پر جب تحقیقات
   چلائیں ؛ دونوں سیریز `dashboards/grafana/docs_portal.json` میں گراف ہیں۔

## مرحلہ 10 - نگرانی اور ثبوت کا بنڈل- ** ڈیش بورڈز۔
  `dashboards/grafana/sorafs_gateway_observability.json` (گیٹ وے لیٹینسی +
  پروف ہیلتھ) ، اور `dashboards/grafana/sorafs_fetch_observability.json`
  (آرکسٹریٹر صحت) ہر رہائی کے لئے۔ JSON برآمدات کو ٹکٹ پر منسلک کریں
  جاری کریں تاکہ جائزہ لینے والے Prometheus سوالات کو دوبارہ پیش کرسکیں۔
- ** تحقیقات فائلیں۔ ** `artifacts/sorafs_gateway_probe/<stamp>/` کو گٹ-اینیکس میں رکھیں
  یا آپ کے ثبوت بالٹی۔ تحقیقات کا خلاصہ ، ہیڈر ، اور پیجریڈی پے لوڈ پر مشتمل ہے
  ٹیلی میٹری اسکرپٹ کے ذریعہ پکڑا گیا۔
- ** ریلیز بنڈل۔
  دستخط Sigstore ، `portal.pin.report.json` ، TRY-IT تحقیقات لاگز ، اور لنک چیک رپورٹس
  ٹائم اسٹیمپ (جیسے `artifacts/sorafs/devportal/20260212T1103Z/`) والے فولڈر کے تحت۔
- ** ڈرل لاگ۔ ** جب تحقیقات کسی ڈرل کا حصہ ہوں تو انہیں اجازت دیں
  `scripts/telemetry/run_sorafs_gateway_probe.sh` `ops/drill-log.md` میں اندراجات شامل کریں
  تاکہ وہی ثبوت SNNET-5 افراتفری کی ضرورت کو پورا کریں۔
- ** ٹکٹ کے لنکس۔
  تبدیلی کی ، تحقیقات کی رپورٹ کے ساتھ ساتھ ، تاکہ جائزہ لینے والے ایس ایل او ایس کو عبور کرسکیں
  کوئی شیل تک رسائی نہیں ہے۔

## مرحلہ 11-ملٹی سورس بازیافت ڈرل اور اسکور بورڈ ثبوت

SoraFS پر پوسٹ کرنے کے لئے اب ملٹی سورس بازیافت کے ثبوت کی ضرورت ہے (DOCS-7/SF-6)
پچھلے DNS/گیٹ وے ٹیسٹ کے ساتھ۔ منشور ترتیب دینے کے بعد:

1. ** براہ راست منشور کے خلاف `sorafs_fetch` چلائیں۔ ** ایک ہی منصوبہ/ظاہر نمونے استعمال کریں
   ہر فراہم کنندہ کے لئے جاری کردہ گیٹ وے کی اسناد 2-3 کے اقدامات میں تیار کی گئی ہے۔ برقرار رہتا ہے
   تمام آؤٹ پٹ تاکہ آڈیٹر آرکسٹریٹر کے فیصلے کا سراغ لگاسکیں:

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

   - منشور کے ذریعہ حوالہ کردہ سپلائرز کے اشتہارات کی تلاش کریں (مثال کے طور پر
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     اور ان کو `--provider-advert name=path` کے ذریعے پاس کریں تاکہ اسکور بورڈ ونڈوز کا اندازہ کرسکے
     صلاحیت کے عزم کے مطابق. استعمال کریں
     `--allow-implicit-provider-metadata` ** صرف ** جب آپ CI میں فکسچر کھیلتے ہیں۔ مشقیں
     پروڈکشن کو لازمی طور پر دستخط شدہ اشتہارات کا حوالہ دینا ہوگا جو پن کے ساتھ پہنچے۔
   - جب ظاہر ہوتا ہے اضافی خطوں کا حوالہ دیتا ہے تو ، کمانڈ کو ٹیوپلس کے ساتھ دہرائیں
     متعلقہ فراہم کنندگان تاکہ ہر کیشے/عرف سے وابستہ بازیافت کا نمونہ ہو۔

2. ** آؤٹ پٹس کو محفوظ کریں۔
   `providers.ndjson` ، `fetch.json` ، اور `chunk_receipts.ndjson` کے ثبوت فولڈر کے تحت
   ریلیز یہ فائلیں ہم مرتبہ وزن ، دوبارہ بجٹ ، EWMA لیٹینسی ، اور پر قبضہ کرتی ہیں
   فی حصہ رسیدیں کہ گورننس پیکیج کو SF-7 کے لئے برقرار رکھنا چاہئے۔

3. ** ٹیلی میٹری کو اپ ڈیٹ کریں۔
   مشاہدہ ** (`dashboards/grafana/sorafs_fetch_observability.json`) ،
   `torii_sorafs_fetch_duration_ms`/`_failures_total` اور سپلائر کے ذریعہ رینج پینلز کا مشاہدہ کرنا
   بے ضابطگیوں کے لئے۔ Grafana پینل اسنیپ شاٹس کو ریلیز کے ٹکٹ کے ساتھ ساتھ
   اسکور بورڈ.4. ** انتباہ کے قواعد تمباکو نوشی کریں۔ ** `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں
   رہائی بند کرنے سے پہلے الرٹ بنڈل Prometheus کی توثیق کرنے کے لئے۔ کی آؤٹ پٹ منسلک کریں
   اس بات کی تصدیق کے ل docs دستاویزات -7 جائزہ لینے والوں کے لئے ٹکٹ پر پرومٹول
   سست فراہم کرنے والا اب بھی مسلح ہے۔

5. ** تار میں تار۔
   `perform_fetch_probe` ؛ اس کو اسٹیجنگ/پروڈکشن رنز کے ل enable قابل بنائیں تاکہ اس کا ثبوت ہو
   بازیافت دستی مداخلت کے بغیر مینی فیسٹ بنڈل کے ساتھ ساتھ تیار کی جاتی ہے۔ مقامی مشقیں کر سکتی ہیں
   گیٹ وے ٹوکن برآمد کرکے اور `PIN_FETCH_PROVIDERS` ترتیب دے کر اسی اسکرپٹ کو دوبارہ استعمال کریں
   سپلائرز کی کوما سے الگ کردہ فہرست میں۔

## تشہیر ، مشاہدہ اور رول بیک

1. ** پروموشن: ** اسٹیجنگ اور پروڈکشن کے لئے الگ الگ عرفیت رکھیں۔ دوبارہ چلانے کو فروغ دیں
   `manifest submit` اسی مینی فیسٹ/بنڈل کے ساتھ ، بدل رہا ہے
   `--alias-namespace/--alias-name` پروڈکشن عرف کی طرف اشارہ کرنے کے لئے۔ یہ گریز کرتا ہے
   ایک بار جب کیو اے اسٹیجنگ پن کی منظوری دے تو دوبارہ تعمیر کریں یا دوبارہ دستخط کریں۔
2. ** نگرانی: ** پن رجسٹری ڈیش بورڈ درآمد کریں
   (`docs/source/grafana_sorafs_pin_registry.json`) پلٹل سے متعلق مخصوص تحقیقات
   (`docs/portal/docs/devportal/observability.md` دیکھیں)۔ چیکسم ڈرفٹ الرٹ ،
   ناکام تحقیقات یا پروف دوبارہ کوششیں کریں۔
3. ** رول بیک: ** واپس رول کرنے کے لئے ، پچھلے مینی فیسٹ (یا موجودہ عرف کو ہٹا دیں) کا استعمال کرتے ہوئے واپس رول کریں
   `sorafs_cli manifest submit --alias ... --retire`۔ ہمیشہ تازہ ترین بنڈل رکھیں
   گڈ اور کار سمری کے نام سے جانا جاتا ہے تاکہ رول بیک ٹیسٹ کو دوبارہ بنایا جاسکے اگر
   سی آئی لاگ کو گھمایا جاتا ہے۔

## CI ورک فلو ٹیمپلیٹ

کم سے کم ، آپ کی پائپ لائن کو چاہئے:

1. بلڈ + لنٹ (`npm ci` ، `npm run build` ، چیکسم جنریشن)۔
2. پیکیج (`car pack`) اور کمپیوٹ مینی فیسٹ۔
3. نوکری ٹوکن OIDC (`manifest sign`) کا استعمال کرتے ہوئے دستخط کریں۔
4. آڈٹ کے لئے نمونے (کار ، منشور ، بنڈل ، منصوبہ ، خلاصہ) اپ لوڈ کریں۔
5. رجسٹری پن پر بھیجیں:
   - درخواستیں کھینچیں -> `docs-preview.sora`۔
   - محفوظ ٹیگز / شاخیں -> پروڈکشن عرفی کو فروغ دینا۔
6. تکمیل سے پہلے تحقیقات + پروف تصدیق کے دروازے چلائیں۔

`.github/workflows/docs-portal-sorafs-pin.yml` ان تمام اقدامات کو دستی ریلیز سے جوڑتا ہے۔
ورک فلو:

- پورٹل کی تعمیر/جانچ ،
- `scripts/sorafs-pin-release.sh` کے ذریعے تعمیر کو پیکج کریں ،
- GITHUB OIDC کا استعمال کرتے ہوئے مینی فیسٹ بنڈل پر دستخط/تصدیق کریں ،
- کار/منشور/بنڈل/پلان/خلاصے کو نمونے کے طور پر اپ لوڈ کریں ، اور
- (اختیاری طور پر) جب راز ہوں تو مینی فیسٹ + عرف بائنڈنگ بھیجیں۔

نوکری کو متحرک کرنے سے پہلے مندرجہ ذیل ذخیرہ راز/متغیرات کی تشکیل کریں:| نام | مقصد |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | میزبان Torii کو بے نقاب `/v1/sorafs/pin/register`۔ |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | گذارشات کے ساتھ رجسٹرڈ ایپچ شناخت کنندہ۔ |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | ظاہر جمع کرانے کے لئے اتھارٹی پر دستخط کرنا۔ |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | `perform_submit` جب `true` ہے تو عرف ٹوپل ظاہر ہونے کا پابند ہے۔ |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | بیس 64 انکوڈڈ عرف پروف بنڈل (اختیاری ؛ عرف بائنڈنگ کو چھوڑنے کے لئے چھوڑ دیا گیا)۔ |
| `DOCS_ANALYTICS_*` | موجودہ تجزیات/تحقیقات کے اختتامی مقامات دوسرے ورک فلوز کے ذریعہ دوبارہ استعمال کیے جاتے ہیں۔ |

کاموں کے ذریعے ورک فلو کو متحرک کریں UI:

1. `alias_label` فراہم کریں (مثال کے طور پر ، `docs.sora.link`) ، `proposal_alias` اختیاری ،
   اور `release_tag` کا ایک اختیاری اوور رائڈ۔
2. چھوڑو `perform_submit` Torii کو چھوئے بغیر نمونے تیار کرنے کے لئے بغیر چیک کیا
   (خشک رنز کے ل useful مفید) یا اسے براہ راست تشکیل شدہ عرف میں شائع کرنے کے قابل بنائیں۔

`docs/source/sorafs_ci_templates.md` اب بھی عام CI مددگاروں کے لئے دستاویز کرتا ہے
اس ریپو سے باہر کے منصوبے ، لیکن پورٹل ورک فلو ترجیحی آپشن ہونا چاہئے
یومیہ ریلیز کے لئے۔

## چیک لسٹ

- [] `npm run build` ، `npm run test:*` ، اور `npm run check:links` سبز رنگ میں ہیں۔
- [] `build/checksums.sha256` اور `build/release.json` نمونے پر قبضہ کیا گیا۔
- [] کار ، منصوبہ ، منشور ، اور خلاصہ `artifacts/` کے تحت تیار کیا گیا ہے۔
- [] بنڈل Sigstore + لاگوں کے ساتھ محفوظ علیحدہ دستخط۔
- [] `portal.manifest.submit.summary.json` اور `portal.manifest.submit.response.json`
      جب گذارشات ہوں تو پکڑا گیا۔
- [] `portal.pin.report.json` (اور اختیاری `portal.pin.proposal.json`)
      کار/منشور نمونے کے ساتھ مل کر محفوظ شدہ۔
- [] `proof verify` اور `manifest verify-signature` آرکائویٹڈ کے نوشتہ جات۔
- [] تازہ ترین Grafana ڈیش بورڈز + کامیاب TRY-IT تحقیقات۔
- [] رول بیک نوٹ (پچھلا مینی فیسٹ ID + عرف ڈائجسٹ) سے منسلک
      جاری ٹکٹ۔