---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## جائزہ

یہ پلے بک روڈ میپ آئٹمز ** دستاویزات -7 ** (اشاعت SoraFS) اور ** دستاویزات -8 ** کو تبدیل کرتی ہے۔
(CI/CD پن آٹومیشن) ڈویلپر پورٹل کے لئے قابل عمل طریقہ کار میں۔
اس میں بلڈ/لنٹ مرحلے ، پیکیجنگ SoraFS کا احاطہ کیا گیا ہے ، Sigstore کے ساتھ ظاہر ہوتا ہے ،
عرف پروموشن ، توثیق اور رول بیک مشقیں تاکہ ہر پیش نظارہ اور ریلیز
تولیدی اور قابل آڈٹیبل ہے۔

بہاؤ فرض کرتا ہے کہ آپ کے پاس `sorafs_cli` بائنری ہے (`--features cli` کے ساتھ مرتب کیا گیا ہے) ، تک رسائی
Torii پن رجسٹری اجازتوں کے ساتھ ، اور Sigstore کے لئے اسناد OIDC کے ساتھ۔
ان میں طویل مدتی راز (`IROHA_PRIVATE_KEY` ، `SIGSTORE_ID_TOKEN` ، ٹوکن Torii) میں اسٹور کریں
آپ کا CI محفوظ ؛ مقامی پھانسی ان کو شیل برآمدات کے ذریعے لوڈ کرسکتی ہے۔

## شرائط

- `npm` یا `pnpm` کے ساتھ نوڈ 18.18+۔
- `sorafs_cli` کے ذریعے `cargo run -p sorafs_car --features cli --bin sorafs_cli`۔
- URL Torii جو `/v1/sorafs/*` کے علاوہ ایک اکاؤنٹ/نجی کلیدی اتھارٹی کو بے نقاب کرتا ہے جو جمع کراسکتا ہے
  منشور اور عرفی۔
- `SIGSTORE_ID_TOKEN` جاری کرنے کے لئے جاری کرنے والا OIDC (گٹ ہب ایکشنز ، گٹ لیب ، کام کے بوجھ کی شناخت ، وغیرہ)۔
- اختیاری: خشک رنز کے لئے `examples/sorafs_cli_quickstart.sh` اور
  `docs/source/sorafs_ci_templates.md` گٹھب/گٹ لیب ورک فلوز کے لئے سہاروں کے لئے۔
- OAuth کو تشکیل دیں اس کی متغیر (`DOCS_OAUTH_*`) کو آزمائیں اور اسے چلائیں
  [سیکیورٹی سخت کرنے والی چیک لسٹ] (./security-hardening.md) کسی تعمیر کو فروغ دینے سے پہلے
  لیب کے باہر جب یہ متغیرات غائب ہوں تو پورٹل بلڈ اب ناکام ہوجاتا ہے
  یا جب مسلط ونڈوز سے ٹی ٹی ایل/پولنگ نوبس نکل آئیں۔ برآمد
  `DOCS_OAUTH_ALLOW_INSECURE=1` صرف ڈسپوز ایبل مقامی پیش نظارہ کے لئے۔ شامل ہوں
  ریلیز ٹکٹ کے قلم ٹیسٹ کے ثبوت۔

## مرحلہ 0 - ایک پراکسی بنڈل پر قبضہ کریں اسے آزمائیں

نیٹ لائی یا گیٹ وے کے پیش نظارہ کو فروغ دینے سے پہلے ، پراکسی ذرائع کو آزمائیں
اور منشور ڈائجسٹ OpenAPI ایک جینیاتی بنڈل میں دستخط شدہ:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` پراکسی/تحقیقات/رول بیک مددگار ، دستخطوں کی جانچ پڑتال کریں
OpenAPI اور `release.json` پلس `checksums.sha256` لکھتا ہے۔ اس بنڈل کو ٹکٹ سے منسلک کریں
نیٹ لیف/SoraFS گیٹ وے پروموشن تاکہ جائزہ لینے والے عین مطابق ذرائع کو دوبارہ چلاسکیں
اور بغیر کسی تعمیر نو کے ہدف Torii کے اشارے۔ بنڈل بھی ریکارڈ کرتا ہے اگر بیئرز
رول آؤٹ پلان کو برقرار رکھنے کے لئے گاہک کے ذریعہ فراہم کردہ (`allow_client_auth`) فعال تھے
اور مرحلے میں سی ایس پی کے قواعد۔

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

- `build/checksums.sha256` - SHA256 `sha256sum -c` کے لئے منشور۔
- `build/release.json` - میٹا ڈیٹا (`tag` ، `generated_at` ، `source`) ہر کار/منشور میں طے شدہ۔

ان فائلوں کو کار سمری کے ساتھ محفوظ شدہ دستاویزات بنائیں تاکہ جائزہ لینے والے نمونے کا موازنہ کرسکیں
تعمیر نو کے بغیر پیش نظارہ۔

## مرحلہ 2 - پیکیج جامد اثاثےآؤٹ پٹ ڈائرکٹری Docusaurus کے خلاف کار پیکر چلائیں۔ ذیل میں لکھی گئی مثال
`artifacts/devportal/` کے تحت نمونے۔

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

JSON کا خلاصہ حصہ گنتی ، ہضم اور پروف پلاننگ کے اشارے پر قبضہ کرتا ہے
کہ `manifest build` اور CI ڈیش بورڈز بعد میں دوبارہ استعمال کریں۔

## مرحلہ 2B - OpenAPI اور SBOM ساتھیوں کو پیکج کریں

DOCS-7 کے لئے پورٹل سائٹ ، اسنیپ شاٹ OpenAPI اور SBOM پے لوڈ کی اشاعت کی ضرورت ہے
علیحدہ ظاہر ہونے کے ناطے تاکہ گیٹ وے ہیڈر کو تیز کرسکیں
`Sora-Proof`/`Sora-Content-CID` ہر نمونے کے لئے۔ رہائی مددگار
(`scripts/sorafs-pin-release.sh`) پہلے ہی ڈائریکٹری OpenAPI پیکجز
(`static/openapi/`) اور SBOMs الگ کاروں میں `syft` کے ذریعے جاری کیا گیا
`openapi.*`/`*-sbom.*` اور میٹا ڈیٹا کو محفوظ کرتا ہے
`artifacts/sorafs/portal.additional_assets.json`۔ دستی بہاؤ میں ،
ہر پے لوڈ کے لئے اپنے میٹا ڈیٹا کے سابقہ ​​اور لیبل کے ساتھ 2-4 اقدامات دہرائیں
(جیسے `--car-out "$OUT"/openapi.car` مزید
`--metadata alias_label=docs.sora.link/openapi`)۔ ہر منشور/عرف جوڑی کو بچائیں
DNS کو تبدیل کرنے سے پہلے Torii (سائٹ ، OpenAPI ، SBOM پورٹل ، SBOM OpenAPI) پر
گیٹ وے تمام شائع شدہ نمونے کے لئے مستحکم ثبوت پیش کرتا ہے۔

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

اپنی ریلیز ونڈو کے مطابق پن پالیسی کے جھنڈوں کو ایڈجسٹ کریں (مثال کے طور پر ، `-پرین اسٹوریج کلاس
کینریوں کے لئے گرم)۔ JSON مختلف حالت اختیاری ہے لیکن کوڈ کے جائزے کے لئے عملی ہے۔

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

بنڈل میں مینی فیسٹ ڈائجسٹ ، حصہ ڈائجسٹ اور ٹوکن کا ایک بلیک 3 ہیش ریکارڈ کیا گیا ہے
OIDC JWT کو برقرار رکھے بغیر۔ بنڈل اور دستخط کو الگ رکھیں۔ کی پروموشنز
پیداوار دوبارہ دستخط کرنے کے بجائے ایک ہی نمونے کو دوبارہ استعمال کرسکتی ہے۔ مقامی پھانسی
فراہم کنندہ کے جھنڈوں کو `--identity-token-env` (یا `SIGSTORE_ID_TOKEN` کی وضاحت کرسکتے ہیں
) جب کوئی بیرونی OIDC مددگار ٹوکن جاری کرتا ہے۔

## مرحلہ 5 - پن رجسٹری میں جمع کروائیں

Torii پر دستخط شدہ منشور (اور حصہ منصوبہ) جمع کروائیں۔ ہمیشہ کے لئے خلاصہ طلب کریں
کہ نتیجے میں ان پٹ/عرف انتہائی قابل عمل ہے۔

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

پیش نظارہ یا کینری رول آؤٹ (`docs-preview.sora`) کے دوران ، جمع کرانے کو دہرائیں
ایک انوکھا عرف کے ساتھ تاکہ QA پیداوار کو فروغ دینے سے پہلے مواد کی تصدیق کرسکے۔

عرف بائنڈنگ کے لئے تین فیلڈز کی ضرورت ہے: `--alias-namespace` ، `--alias-name` اور `--alias-proof`۔
جب عرفی درخواست ہوتی ہے تو گورننس پروف بنڈل (بیس 64 یا بائٹس Norito) تیار کرتا ہے
منظور شدہ ؛ اسے CI رازوں میں اسٹور کریں اور طلب کرنے سے پہلے اسے فائل کے طور پر بے نقاب کریں
`manifest submit`۔ جب آپ صرف پن کرنا چاہتے ہیں تو عرف پرچم خالی چھوڑ دیں
ڈی این ایس کو چھوئے بغیر اسے ظاہر کرتا ہے۔

## مرحلہ 5B - گورننس کی تجویز تیار کریں

ہر منشور کو پارلیمنٹ کے لئے تیار تجویز کے ساتھ سفر کرنا چاہئے تاکہ ہر شہری
سورہ مراعات یافتہ اسناد کے بغیر تبدیلی کو متعارف کراسکتی ہے۔ کے بعد
جمع کروائیں/سائن اقدامات ، چلائیں:```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` نے کیننیکل انسٹرکشن `RegisterPinManifest` کو اپنی گرفت میں لے لیا ،
حصہ ڈائجسٹ ، پالیسی اور عرف انڈیکس۔ اسے گورننس ٹکٹ سے منسلک کریں یا
پارلیمنٹ کا پورٹل تاکہ مندوبین بغیر کسی تعمیر نو کے پے لوڈ کا موازنہ کرسکیں۔
چونکہ کمانڈ کبھی بھی اتھارٹی کی کلید Torii کو چھو نہیں لیتی ہے ، کوئی بھی شہری لکھ سکتا ہے
مقامی طور پر تجویز.

## مرحلہ 6 - ثبوت اور ٹیلی میٹری چیک کریں

پن کے بعد ، تشخیصی تصدیق کے اقدامات پر عمل کریں:

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

- `torii_sorafs_gateway_refusals_total` اور نگرانی کریں
  `torii_sorafs_replication_sla_total{outcome="missed"}` بے ضابطگیوں کے لئے۔
- `npm run probe:portal` کو چلانے کی کوشش کریں تاکہ پراکسی اور محفوظ کردہ لنکس کو استعمال کیا جاسکے
  تازہ پنن مواد کے خلاف۔
- میں بیان کردہ نگرانی کے شواہد پر قبضہ کریں
  [پبلشنگ اینڈ مانیٹرنگ] (./publishing-monitoring.md) تاکہ مشاہدہ گیٹ
  DOCS-3C اشاعت کے اقدامات سے مطمئن ہے۔ مددگار اب قبول کرتا ہے
  کئی اندراجات `bindings` (سائٹ ، OpenAPI ، SBOM پورٹل ، SBOM OpenAPI) اور نافذ کریں
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` اختیاری گارڈ کے ذریعہ ہدف میزبان پر
  `hostname`۔ ذیل میں درخواست ایک واحد JSON سمری اور ثبوت کے بنڈل لکھتی ہے
  ڈائریکٹری کے تحت (`portal.json` ، `tryit.json` ، `binding.json` ، `checksums.sha256`)
  ریلیز کی:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## مرحلہ 6A - گیٹ وے سرٹیفکیٹ کی منصوبہ بندی کریں

گار پیکٹ بنانے سے پہلے سان/TLS چیلنج پلان سے اخذ کریں تاکہ گیٹ وے ٹیم اور
ڈی این ایس کے منظوری ایک ہی ثبوت پر نظر ڈالتی ہے۔ نیا مددگار DG-3 آدانوں کی عکاسی کرتا ہے
کیننیکل وائلڈ کارڈ کے میزبانوں ، خوبصورت میزبان سینز ، DNS-01 لیبلوں کی فہرست اور
ACME چیلنجز تجویز کرتے ہیں:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON کو ریلیز کے بنڈل (یا اسے تبدیلی کے ٹکٹ کے ساتھ اپ لوڈ کریں) کے ساتھ ارتکاب کریں
تاکہ آپریٹرز SAN اقدار کو ترتیب میں چسپاں کرسکیں
`torii.sorafs_gateway.acme` of Torii اور یہ کہ GAR جائزہ لینے والے اس کی تصدیق کرسکتے ہیں
میزبان اخذات کو دوبارہ چلانے کے بغیر کیننیکل/خوبصورت نقشہ سازی۔ دلائل شامل کریں
اسی ریلیز میں ترقی یافتہ ہر لاحقہ کے لئے اضافی `--name`۔

## مرحلہ 6 بی - کیننیکل میزبان میپنگز اخذ کریں

GAR پے لوڈ کو ٹیمپلیٹ کرنے سے پہلے ، ہر عرف کے لئے ڈٹرمینسٹک میزبان نقشہ سازی کو ریکارڈ کریں۔
`cargo xtask soradns-hosts` ہر `--name` کو اس کے کیننیکل لیبل میں ہیش کرتا ہے
(`<base32>.gw.sora.id`) ، مطلوبہ وائلڈ کارڈ (`*.gw.sora.id`) جاری کرتا ہے اور خوبصورت میزبان کو حاصل کرتا ہے
(`<alias>.gw.sora.name`)۔ ریلیز نمونے میں آؤٹ پٹ کو برقرار رکھیں تاکہ
DG-3 جائزہ لینے والے GAR جمع کرانے کے ساتھ نقشہ سازی کا موازنہ کرسکتے ہیں:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

جب GAR یا JSON جب جلدی ناکام ہونے کے لئے `--verify-host-patterns <file>` استعمال کریں
بائنڈنگ گیٹ وے مطلوبہ میزبانوں میں سے ایک کو چھوڑ دیتا ہے۔ مددگار کئی توثیق فائلوں کو قبول کرتا ہے ،
جس سے GAR ٹیمپلیٹ اور `portal.gateway.binding.json` اسٹیپل کو لنٹ کرنا آسان ہوجاتا ہے
ایک ہی دعوت:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```دوبارہ شروع JSON اور تصدیق شدہ لاگ ان DNS/گیٹ وے تبدیلی کے ٹکٹ پر منسلک کریں تاکہ
سامعین کیننیکل ، وائلڈ کارڈ اور خوبصورت میزبانوں کی تصدیق کے بغیر ان کی تصدیق کرسکتے ہیں
اسکرپٹس۔ کمانڈ کو دوبارہ چلائیں جب بنڈل میں نئے عرفات کو شامل کیا جائے تاکہ
گار کی تازہ کارییں ایک ہی ثبوت کے وارث ہیں۔

## مرحلہ 7 - DNS کٹ اوور ڈسریکٹر تیار کریں

پروڈکشن کٹ اوور کے لئے ایک آڈٹیبل چینج پیکیج کی ضرورت ہوتی ہے۔ جمع کرانے کے بعد
کامیاب (عرف بائنڈنگ) ، مددگار اخراج کرتا ہے
`artifacts/sorafs/portal.dns-cutover.json` ، گرفتاری:

- عرف بائنڈنگ میٹا ڈیٹا (نام کی جگہ/نام/ثبوت ، ڈائجسٹ منشور ، یو آر ایل Torii ،
  عہد پیش کیا گیا ، اتھارٹی) ؛
- ریلیز سیاق و سباق (ٹیگ ، عرف لیبل ، منشور/کار کے راستے ، چنک پلان ، بنڈل Sigstore) ؛
- توثیق کے اشارے (تحقیقات کمانڈ ، عرف + اختتامی نقطہ Torii) ؛ اور
- اختیاری تبدیلی پر قابو پانے والے فیلڈز (ٹکٹ ID ، کٹ اوور ونڈو ، OPS رابطہ ،
  میزبان نام/پروڈکشن زون) ؛
- روٹ پروموشن میٹا ڈیٹا ہیڈر `Sora-Route-Binding` سے ماخوذ ہے
  (کیننیکل میزبان/سی آئی ڈی ، ہیڈر + پابند راستے ، توثیق کے احکامات) ، اس بات کو یقینی بناتے ہوئے
  GAR پروموشن اور فال بیک مشقیں ایک ہی ثبوت کا حوالہ دیتی ہیں۔
- روٹ پلان کے نمونے تیار کردہ (`gateway.route_plan.json` ،
  ہیڈر ٹیمپلیٹس اور اختیاری رول بیک ہیڈر) تاکہ ٹکٹوں کو تبدیل کریں اور
  CI لنٹ ہکس تصدیق کر سکتے ہیں کہ ہر DG-3 پیکیج کا حوالہ دیتا ہے
  منظوری سے پہلے کیننیکل پروموشن/رول بیک ؛
- اختیاری کیشے باطل میٹا ڈیٹا (پرج اختتامی نقطہ ، آتھ متغیر ، JSON پے لوڈ ،
  اور مثال کے طور پر کمانڈ `curl`) ؛ اور
- رول بیک اشارے پچھلے ڈسکرپٹر کی طرف اشارہ کرتے ہیں (ریلیز ٹیگ اور ڈائجسٹ مینی فیسٹ)
  ایک عین مطابق فال بیک بیک راستے پر قبضہ کرنے کے لئے ٹکٹوں کے لئے۔

جب رہائی میں کیشے صاف کرنے کی ضرورت ہوتی ہے تو ، ڈسکرپٹر کے ساتھ ایک کیننیکل پلان تیار کریں:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

`portal.cache_plan.json` کو DG-3 پیکیج سے منسلک کریں تاکہ آپریٹرز کے پاس میزبان/راستے ہوں
 جب وہ `PURGE` درخواستیں جاری کرتے ہیں تو تعی .ن کرنے والے (اور اسی سے متعلقہ اشارے)۔
ڈسکرپٹر کا اختیاری کیشے سیکشن اس فائل کا براہ راست حوالہ دے سکتا ہے ،
جائزہ لینے والے کٹ اوور کے دوران بالکل صاف ہونے والے اختتامی مقامات پر متفق ہیں۔

ہر DG-3 پیکیج کو بھی پروموشن + رول بیک چیک لسٹ کی ضرورت ہوتی ہے۔ اس کے ذریعے پیدا کریں
`cargo xtask soradns-route-plan` تاکہ جائزہ لینے والے عین مطابق اقدامات کا سراغ لگاسکیں
عرف کے ذریعہ پریفلائٹ ، کٹ اوور اور رول بیک:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` نے کیننیکل/خوبصورت میزبانوں ، صحت کی جانچ پڑتال کی یاد دہانیوں کو جاری کیا
قدم کے لحاظ سے ، GAR پابند اپڈیٹس ، کیشے صاف اور رول بیک ایکشنز۔ اسے منسلک کریں
تبدیلی کا ٹکٹ جمع کروانے سے پہلے گار/بائنڈنگ/کٹ اوور نمونے
اسی اسکرپٹ اقدامات کو دہرائیں اور توثیق کریں۔

`scripts/generate-dns-cutover-plan.mjs` اس ڈسکرپٹر کو کھانا کھلاتا ہے اور خود بخود عمل میں لاتا ہے
`sorafs-pin-release.sh`۔ اسے دستی طور پر دوبارہ تخلیق کرنا یا اپنی مرضی کے مطابق بنانا:

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
```پن مددگار چلانے سے پہلے ماحولیاتی متغیر کے ذریعہ اختیاری میٹا ڈیٹا فراہم کریں:

| مختلف ہوتا ہے | مقصد |
| --- | --- |
| `DNS_CHANGE_TICKET` | ڈسکرپٹر میں ٹکٹ کی شناخت محفوظ ہے۔ |
| `DNS_CUTOVER_WINDOW` | ISO8601 کٹ اوور ونڈو (مثال کے طور پر: `2026-03-21T15:00Z/2026-03-21T15:30Z`)۔ |
| `DNS_HOSTNAME` ، `DNS_ZONE` | میزبان نام کی پیداوار + مستند زون۔ |
| `DNS_OPS_CONTACT` | عرف آن کال یا بڑھتی ہوئی رابطہ۔ |
| `DNS_CACHE_PURGE_ENDPOINT` | اختتامی نقطہ پرج کیشے نے وضاحت کرنے والے میں بچایا ہے۔ |
| `DNS_CACHE_PURGE_AUTH_ENV` | پرج ٹوکن (پہلے سے طے شدہ: `CACHE_PURGE_TOKEN`) پر مشتمل var var۔ |
| `DNS_PREVIOUS_PLAN` | میٹا ڈیٹا رول بیک کے لئے پچھلے ڈسکرپٹر کا راستہ۔ |

JSON کو DNS تبدیلی کے جائزے سے منسلک کریں تاکہ منظوری دینے والے ہضم کی تصدیق کرسکیں
سی آئی لاگز کے ذریعے کھودے بغیر منشور ، عرف بائنڈنگ اور تحقیقات کے احکامات۔ سی ایل آئی جھنڈے
`--dns-change-ticket` ، `--dns-cutover-window` ، `--dns-hostname` ،
`--dns-zone` ، `--ops-contact` ، `--cache-purge-endpoint` ،
`--cache-purge-auth-env` ، اور `--previous-dns-plan` وہی اوور رائڈس فراہم کرتا ہے
جب مددگار CI کے باہر بھاگتا ہے۔

## مرحلہ 8 - حل کرنے والے زون فائل کنکال (اختیاری) کا اخراج کریں

جب پروڈکشن کٹ اوور ونڈو معلوم ہوجائے تو ، ریلیز اسکرپٹ خارج ہوسکتی ہے
ایس این ایس زون فائل کنکال اور اسنیپٹ کو خود بخود حل کرنے والا۔ ان کو پاس کرو
ماحولیاتی متغیرات یا سی ایل آئی کے اختیارات کے ذریعہ DNS خواہشات اور میٹا ڈیٹا ریکارڈ کرتا ہے۔ مددگار
ڈسکرپٹر تیار کرنے کے فورا بعد `scripts/sns_zonefile_skeleton.py` کال کرتا ہے۔
کم از کم ایک A/AAAA/CNAME ویلیو اور GAR ڈائجسٹ (GAR پے لوڈ سائن کا BLAKE3-256) فراہم کریں۔
اگر زون/میزبان نام جانا جاتا ہے اور `--dns-zonefile-out` کو چھوڑ دیا جاتا ہے تو ، مددگار لکھتا ہے
`artifacts/sns/zonefiles/<zone>/<hostname>.json` اور بھرتا ہے
`ops/soradns/static_zones.<hostname>.json` بطور ٹکڑا حل کرنے والا۔| متغیر/پرچم | مقصد |
| --- | --- |
| `DNS_ZONEFILE_OUT` ، `--dns-zonefile-out` | پیدا شدہ زون فائل کنکال کا راستہ۔ |
| `DNS_ZONEFILE_RESOLVER_SNIPPET` ، `--dns-zonefile-resolver-snippet` | اسنیپٹ ریزولور کا راستہ (پہلے سے طے شدہ: `ops/soradns/static_zones.<hostname>.json` اگر چھوڑ دیا گیا ہو)۔ |
| `DNS_ZONEFILE_TTL` ، `--dns-zonefile-ttl` | ٹی ٹی ایل تیار کردہ ریکارڈز (پہلے سے طے شدہ: 600 سیکنڈ) پر لاگو ہوتا ہے۔ |
| `DNS_ZONEFILE_IPV4` ، `--dns-zonefile-ipv4` | IPv4 پتے (کوما یا تکرار کرنے والے CLI پرچم کے ذریعہ الگ الگ)۔ |
| `DNS_ZONEFILE_IPV6` ، `--dns-zonefile-ipv6` | IPv6 پتے۔ |
| `DNS_ZONEFILE_CNAME` ، `--dns-zonefile-cname` | اختیاری CNAME ہدف۔ |
| `DNS_ZONEFILE_SPKI` ، `--dns-zonefile-spki-pin` | SPKI SHA-256 (BASE64) پن۔ |
| `DNS_ZONEFILE_TXT` ، `--dns-zonefile-txt` | اضافی TXT اندراجات (`key=value`)۔ |
| `DNS_ZONEFILE_VERSION` ، `--dns-zonefile-version` | زونفائل ورژن لیبل کے اوور رائڈ کا حساب کتاب ہوتا ہے۔ |
| `DNS_ZONEFILE_EFFECTIVE_AT` ، `--dns-zonefile-effective-at` | کٹ اوور ونڈو کے آغاز کے بجائے ٹائم اسٹیمپ `effective_at` (RFC3339) پر مجبور کرتا ہے۔ |
| `DNS_ZONEFILE_PROOF` ، `--dns-zonefile-proof` | میٹا ڈیٹا میں محفوظ ہونے والے ثبوت کے بارے میں لفظی حد سے زیادہ۔ |
| `DNS_ZONEFILE_CID` ، `--dns-zonefile-cid` | میٹا ڈیٹا میں سی آئی ڈی اوور رائڈ بچت ہے۔ |
| `DNS_ZONEFILE_FREEZE_STATE` ، `--dns-zonefile-freeze-state` | گارڈین کی حیثیت کو منجمد کریں (نرم ، سخت ، پگھلنا ، نگرانی ، ہنگامی صورتحال) |
| `DNS_ZONEFILE_FREEZE_TICKET` ، `--dns-zonefile-freeze-ticket` | ریفرنس ٹکٹ گارڈین/کونسل برائے منجمد۔ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT` ، `--dns-zonefile-freeze-expires-at` | پگھلنے کے لئے ٹائم اسٹیمپ RFC3339۔ |
| `DNS_ZONEFILE_FREEZE_NOTES` ، `--dns-zonefile-freeze-note` | اضافی منجمد نوٹ (کوما یا تکرار کرنے والے پرچم کے ذریعہ الگ الگ)۔ |
| `DNS_GAR_DIGEST` ، `--dns-gar-digest` | GAR پے لوڈ سائن کے بلیک 3-256 (ہیکس) کو ہضم کریں۔ جب گیٹ وے پابندیاں موجود ہوں تو ضروری ہے۔ |

گٹ ہب ایکشن ورک فلو ان اقدار کو ریپو رازوں سے پڑھتا ہے تاکہ ہر پن تیار کرے
خود بخود زون فائل نمونے کا اخراج کرتا ہے۔ مندرجہ ذیل رازوں کو تشکیل دیں (اقدار ہوسکتی ہیں
ملٹی ویلیو فیلڈز کے لئے کوما سے الگ فہرستوں پر مشتمل ہے):

| خفیہ | مقصد |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME` ، `DOCS_SORAFS_DNS_ZONE` | میزبان نام/پروڈکشن زون مددگار کو گزرتا ہے۔ |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | عرف وضاحتی میں کال آن کال۔ |
| `DOCS_SORAFS_ZONEFILE_IPV4` ، `DOCS_SORAFS_ZONEFILE_IPV6` | IPv4/IPv6 ریکارڈ شائع کیے جائیں گے۔ |
| `DOCS_SORAFS_ZONEFILE_CNAME` | اختیاری CNAME ہدف۔ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | ایس پی کے آئی بیس 64 پنوں۔ |
| `DOCS_SORAFS_ZONEFILE_TXT` | اضافی TXT اندراجات۔ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | میٹا ڈیٹا منجمد کنکال میں محفوظ ہوا۔ |
| `DOCS_SORAFS_GAR_DIGEST` | گار پے لوڈ سائن کے ہیکس میں بلیک 3 کو ہضم کریں۔ |

`.github/workflows/docs-portal-sorafs-pin.yml` کو متحرک کرکے ، آدانوں کو فراہم کریں
`dns_change_ticket` اور `dns_cutover_window` تاکہ ڈسکرپٹر/زون فائل کو وراثت میں مل جائے
اچھی ونڈو انہیں صرف خشک رنز کے لئے خالی چھوڑ دیں۔

عام درخواست (SN-7 رن بک کے مالک کے مطابق):

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

مددگار خود بخود تبدیلی کے ٹکٹ کو TXT اندراج کے طور پر بازیافت کرتا ہے اور شروع سے ہی وراثت میں ہوتا ہے
کٹور ونڈو جیسے ٹائم اسٹیمپ `effective_at` کو اوور رائڈ کے علاوہ۔ مکمل ورک فلو کے لئے ، دیکھیں
`docs/source/sorafs_gateway_dns_owner_runbook.md`۔

### عوامی DNS وفد پر نوٹزون فائل کنکال صرف اس کے مستند ریکارڈوں کی وضاحت کرتا ہے
رقبہ آپ کو ابھی بھی پیرنٹ زون کے NS/DS وفد کو تشکیل دینے کی ضرورت ہے
سرورز تلاش کرنے کے لئے عوامی انٹرنیٹ کے لئے آپ کا رجسٹرار یا DNS فراہم کنندہ
ناموں کے

۔
  گیٹ وے کے کسی بھی کاسٹ آئی پی کی طرف اشارہ کرتے ہوئے A/AAAA ریکارڈ شائع کریں۔
- سب ڈومینز کے لئے ، خوبصورت میزبان مشتق کو ایک cname شائع کریں
  (`<fqdn>.gw.sora.name`)۔
- کیننیکل میزبان (`<hash>.gw.sora.id`) گیٹ وے کے ڈومین کے تحت رہتا ہے اور
  آپ کے عوامی زون میں شائع نہیں ہوا ہے۔

### گیٹ وے ہیڈر ٹیمپلیٹ

تعینات مددگار `portal.gateway.headers.txt` اور بھی خارج کرتا ہے
`portal.gateway.binding.json` ، دو نمونے جو DG-3 کی ضرورت کو پورا کرتے ہیں
گیٹ وے-مشمولات کے پابند ہونے کے لئے:

- `portal.gateway.headers.txt` میں HTTP ہیڈرز کا مکمل بلاک (بشمول شامل ہے
  `Sora-Name` ، `Sora-Content-CID` ، `Sora-Proof` ، CSP ، HSTS ، اور تفصیل کار
  `Sora-Route-Binding`) کہ ایج گیٹ ویز کو ہر جواب کے ساتھ منسلک کرنا ہوگا۔
- `portal.gateway.binding.json` مشین سے پڑھنے کے قابل شکل میں ایک ہی معلومات کو ریکارڈ کرتا ہے
  لہذا ٹکٹوں کو تبدیل کریں اور آٹومیشن موازنہ کرسکتے ہیں
  شیل آؤٹ پٹ کو کھرچنے کے بغیر میزبان/سی آئی ڈی بائنڈنگ۔

وہ خود بخود پیدا ہوجاتے ہیں
`cargo xtask soradns-binding-template`
اور عرف پر قبضہ کریں ، ظاہر ڈائجسٹ اور میزبان نام گیٹ وے کو فراہم کیا گیا ہے
`sorafs-pin-release.sh`۔ ہیڈر بلاک کو دوبارہ تخلیق کرنے یا اپنی مرضی کے مطابق بنانے کے لئے ، چلائیں:

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
ڈیفالٹ ہیڈر ٹیمپلیٹس جب کسی تعیناتی کی ہدایت کی ضرورت ہوتی ہے
اضافی ؛ ہٹانے کے لئے موجودہ `--no-*` سوئچ کے ساتھ جوڑیں
مکمل طور پر ایک ہیڈر۔

سی ڈی این تبدیلی کی درخواست کے ساتھ ہیڈر کے ٹکڑوں کو منسلک کریں اور JSON دستاویز کو کھانا کھلائیں
گیٹ وے آٹومیشن پائپ لائن پر تاکہ میزبان پروموشن میچ ہو
رہائی کا ثبوت۔

ریلیز اسکرپٹ خود بخود تصدیق کے مددگار کو انجام دیتا ہے تاکہ
DG-3 ٹکٹوں میں ہمیشہ حالیہ ثبوت شامل ہوتے ہیں۔ اسے دستی طور پر دوبارہ چلائیں
آپ ہاتھ سے پابند JSON میں ترمیم کریں:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

کمانڈ پے لوڈ `Sora-Proof` کلپ کو ضابطہ کشائی کرتا ہے ، اس بات کی تصدیق کرتا ہے کہ میٹا ڈیٹا
`Sora-Route-Binding` مینی فیسٹ سی آئی ڈی + میزبان نام سے مماثل ہے ، اور تیزی سے ناکام ہوجاتا ہے
اگر ایک ہیڈر اخذ کرتا ہے۔ کنسول آؤٹ پٹ کے دیگر نمونے کے ساتھ محفوظ شدہ دستاویزات
تعیناتی جب آپ CI کے باہر کمانڈ چلاتے ہیں تاکہ DG-3 جائزہ نگار
اس بات کا ثبوت ہے کہ کٹ اوور سے پہلے پابند ہونے کی توثیق کی گئی تھی۔> ** DNS ڈسکرپٹر انضمام: ** `portal.dns-cutover.json` اب جہاز
> ایک سیکشن `gateway_binding` ان نمونے کی طرف اشارہ کرتا ہے (راستے ، مواد سی آئی ڈی ،
> ثبوت کی حیثیت اور لغوی ہیڈر ٹیمپلیٹ) ** اور ** ایک اسٹینزا `route_plan`
> کون سا حوالہ `gateway.route_plan.json` نیز ہیڈر ٹیمپلیٹس
> مین اور رول بیک۔ ان بلاکس کو ہر DG-3 ٹکٹ میں شامل کریں تاکہ
> جائزہ لینے والے عین مطابق اقدار `Sora-Name/Sora-Proof/CSP` اور موازنہ کرسکتے ہیں
> تصدیق کریں کہ پروموشن/رول بیک منصوبے ثبوت کے بنڈل کے مطابق ہیں
> بلڈ آرکائیو کھولے بغیر۔

## مرحلہ 9 - ریلیز مانیٹر چلائیں

** دستاویزات -3 سی ** روڈ میپ آئٹم کے لئے مستقل ثبوت کی ضرورت ہوتی ہے کہ پورٹل ، کوشش کریں پراکسی اور
رہائی کے بعد گیٹ وے پابندیاں صحت مند رہیں۔ مستحکم مانیٹر چلائیں
صرف 7-8 مراحل پر عمل کریں اور اسے اپنے تحقیقات کے پروگراموں میں پلگ ان کریں:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` کنفگ فائل کو لوڈ کرتا ہے (دیکھیں
  آریھ کے لئے `docs/portal/docs/devportal/publishing-monitoring.md`) اور
  تین چیکوں پر عمل کریں: پورٹل پاتھ پروبس + سی ایس پی/اجازت-پالیسی کی توثیق ،
  پراکسی تحقیقات اسے آزمائیں (اختیاری طور پر اس کے اختتامی نقطہ `/metrics` کے ذریعے) ، اور تصدیق کنندہ
  بائنڈنگ گیٹ وے (`cargo xtask soradns-verify-binding`) جس کی اب ضرورت ہے
  عرف/مینی فیسٹ چیکوں کے ساتھ Sora-content-CID کی موجودگی اور متوقع قیمت۔
- کمانڈ غیر صفر لوٹاتا ہے جب CI ، CRON ملازمتوں ، یا کی تحقیقات ناکام ہوجاتی ہیں
  رن بک آپریٹر عرفی ناموں کو فروغ دینے سے پہلے ریلیز کو روک سکتے ہیں۔
- پاس `--json-out` ہر ہدف کی حیثیت کے ساتھ ایک JSON خلاصہ لکھتا ہے۔ `--evidence-dir`
  `summary.json` ، `portal.json` ، `tryit.json` ، `binding.json` ، اور `checksums.sha256` کا اخراج کرتا ہے
  تاکہ گورننس کے جائزہ لینے والے مانیٹر کو دوبارہ شروع کیے بغیر نتائج کا موازنہ کرسکیں۔
  بنڈل Sigstore کے ساتھ `artifacts/sorafs/<tag>/monitoring/` کے تحت اس ڈائرکٹری کو محفوظ کریں
  اور DNS ڈسکرپٹر۔
- مانیٹر آؤٹ پٹ شامل کریں ، Grafana (`dashboards/grafana/docs_portal.json`) برآمد کریں ،
  اور ریلیز ٹکٹ میں الرٹ مینجر ڈرل ID تاکہ DOCS-3C SLO ہو
  بعد میں قابل اظہار مانیٹرنگ پبلشنگ پلے بک کے لئے وقف ہے
  `docs/portal/docs/devportal/publishing-monitoring.md`۔

پورٹل تحقیقات میں HTTPS کی ضرورت ہوتی ہے اور بیس URLS `http://` کو مسترد کرنا ہوتا ہے جب تک
`allowInsecureHttp` مانیٹر کنفیگ میں بیان کیا گیا ہے۔ پیداوار/اسٹیجنگ اہداف کو برقرار رکھیں
TLS سے زیادہ اور صرف مقامی پیش نظاروں کے لئے اوور رائڈ کو قابل بنائیں۔

ایک بار بلڈکائٹ/کرون میں `npm run monitor:publishing` کے ذریعے مانیٹر کو خود کار بنائیں
آن لائن پورٹل۔ اسی کمانڈ نے ، پروڈکشن یو آر ایل کی طرف اشارہ کیا ، چیکوں کو کھانا کھلانا ہے
ریلیز کے مابین مستقل صحت۔

## `sorafs-pin-release.sh` کے ساتھ آٹومیشن

`docs/portal/scripts/sorafs-pin-release.sh` اقدامات 2-6 سے منسلک کرتا ہے۔ وہ:1. آرکائیو `build/` ایک ڈٹرمینسٹک ٹربال میں ،
2. عمل کریں
   اور `proof verify` ،
3. اختیاری طور پر `manifest submit` (عرف بائنڈنگ سمیت) پر عمل کریں
   اسناد Torii موجود ہیں ، اور
4. `artifacts/sorafs/portal.pin.report.json` لکھتا ہے ، اختیاری
  `portal.pin.proposal.json` ، DNS ڈسکرپٹر (جمع کرانے کے بعد) ،
  اور بائنڈنگ گیٹ وے بنڈل (`portal.gateway.binding.json` پلس ہیڈر بلاک)
  تاکہ گورننس ، نیٹ ورکنگ اور او پی ایس ٹیمیں شواہد کا موازنہ کرسکیں
  CI لاگز کو تلاش کیے بغیر۔

`PIN_ALIAS` ، `PIN_ALIAS_NAMESPACE` ، `PIN_ALIAS_NAME` ، اور (اختیاری طور پر) سیٹ کریں
اسکرپٹ کی درخواست کرنے سے پہلے `PIN_ALIAS_PROOF_PATH`۔ خشک کیلئے `--skip-submit` استعمال کریں
رنز ؛ ذیل میں گٹ ہب ورک فلو `perform_submit` انٹری کے ذریعے اس کو ٹوگل کرتا ہے۔

## مرحلہ 8 - OpenAPI چشمی اور SBOM بنڈل شائع کریں

دستاویزات -7 کو پورٹل بلڈ ، اسپیشل OpenAPI اور SBOM نمونے کی ضرورت ہوتی ہے
وہی ڈٹرمینسٹک پائپ لائن۔ موجودہ مددگار تینوں کا احاطہ کرتے ہیں:

1. ** دوبارہ تخلیق کریں اور اس پر دستخط کریں۔ **

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   جب آپ اسنیپ شاٹ کو محفوظ رکھنا چاہتے ہیں تو `--version=<label>` کے ذریعے ریلیز کا لیبل فراہم کریں
   تاریخ (جیسے `2025-q3`)۔ مددگار اسنیپ شاٹ لکھتا ہے
   `static/openapi/versions/<label>/torii.json` ، آئینہ in
   `versions/current` ، اور میٹا ڈیٹا کو بچاتا ہے (SHA-256 ، مینی فیسٹ حیثیت ، ٹائم اسٹیمپ
   تازہ ترین) `static/openapi/versions.json` میں۔ ڈویلپر پورٹل اس انڈیکس کو پڑھتا ہے
   تاکہ سویگر/ریپڈوک پینل ایک ورژن چننے والا پیش کریں اور ڈسپلے کریں
   ڈائجسٹ/وابستہ ان لائن دستخط۔ `--version` کو چھوڑ دینا ریلیز لیبل رکھتا ہے
   نظیر اور صرف پوائنٹرز `current` + `latest` کو تازہ دم کرتا ہے۔

   مینی فیسٹ نے SHA-256/BLAKE3 ڈائجسٹس کو اپنی گرفت میں لے لیا تاکہ گیٹ وے اسٹپل ہوسکے
   `/reference/torii-swagger` کے لئے ہیڈر `Sora-Proof`۔

2. ** سائکلونڈ ایکس ایس بی او ایم جاری کرنا۔ ** ریلیز پائپ لائن پہلے ہی سیفٹ ایس بی او ایم کا انتظار کر رہی ہے
   `docs/source/sorafs_release_pipeline_plan.md` کے مطابق۔ باہر نکلیں
   تعمیراتی نمونے کے قریب:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. ** کار میں ہر پے لوڈ کو پیکیج کریں۔ **

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

   مرکزی سائٹ کی طرح اسی اقدامات `manifest build` / `manifest sign` پر عمل کریں ،
   نمونے کے ذریعہ عرفی کو ایڈجسٹ کرکے (مثال کے طور پر ، `docs-openapi.sora` کے لئے اور
   `docs-sbom.sora` SBOM بنڈل سائن کے لئے)۔ عرفی کو الگ الگ برقرار رکھنا
   سوردنس ، گارس اور رول بیک ٹکٹ عین مطابق پے لوڈ تک محدود ہیں۔

4. ** جمع کروائیں اور پابند ہوں۔ ** موجودہ اتھارٹی کو دوبارہ استعمال کریں اور بنڈل Sigstore ، لیکن محفوظ کریں
   ریلیز چیک لسٹ میں عرف ٹپلس تاکہ سامعین ٹریک کرسکیں
   نام سورہ کے نقشے جن میں ہضم ہوتا ہے۔

آرکائیو اسپیشل/ایس بی او ایم بلڈ پورٹل کے ساتھ ظاہر ہوتا ہے اس بات کو یقینی بناتا ہے کہ ہر ریلیز ٹکٹ
پیکر کو دوبارہ چلائے بغیر نمونے کا مکمل سیٹ پر مشتمل ہے۔

### آٹومیشن ہیلپر (CI/پیکیج اسکرپٹ)

`./ci/package_docs_portal_sorafs.sh` اقدامات 1-8 کو کوڈف کرتا ہے تاکہ روڈ میپ آئٹم
** دستاویزات -7 ** ایک ہی کمانڈ کے ساتھ قابل عمل ہے۔ مددگار:- پورٹل (`npm ci` ، SYNC OpenAPI/نوریٹو ، ویجیٹ ٹیسٹ) کی مطلوبہ تیاری پر عمل کریں۔
- پورٹل کے کاروں اور ظاہر جوڑے ، OpenAPI اور SBOM کے ذریعے `sorafs_cli` کا اخراج ؛
- اختیاری طور پر `sorafs_cli proof verify` (`--proof`) اور دستخط Sigstore پر عمل کریں
  (`--sign` ، `--sigstore-provider` ، `--sigstore-audience`) ؛
- تمام نمونے `artifacts/devportal/sorafs/<timestamp>/` اور کے تحت جمع کریں
  `package_summary.json` لکھا تاکہ CI/ریلیز ٹولنگ بنڈل کو کھا سکے۔ اور
- آخری پھانسی کی طرف اشارہ کرنے کے لئے `artifacts/devportal/sorafs/latest` کو تازہ کرتا ہے۔

مثال (Sigstore + POR کے ساتھ مکمل پائپ لائن):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

مفید جھنڈے:

- `--out <dir>` - جڑ کے نمونے کا اوور رائڈ (ڈیفالٹ ٹائم اسٹیمپ فولڈرز رکھتا ہے)۔
- `--skip-build` - موجودہ `docs/portal/build` (جب CI نہیں کرسکتا ہے تو مفید ہے
  آف لائن آئینے کی وجہ سے دوبارہ تعمیر)۔
- `--skip-sync-openapi` - `npm run sync-openapi` کو نظرانداز کرتا ہے جب `cargo xtask openapi`
  کریٹس تک نہیں پہنچ سکتا۔
- `--skip-sbom` - جب بائنری انسٹال نہیں ہوتا ہے تو `syft` کو فون کرنے سے گریز کرتا ہے (اسکرپٹ لاگز
  اس کے بجائے ایک انتباہ)۔
- `--proof` - ہر کار/منشور جوڑی کے لئے `sorafs_cli proof verify` پر عمل کریں۔ پے لوڈ
  ملٹی فائلوں کو ابھی بھی سی ایل آئی میں چنک پلان کی حمایت کی ضرورت ہے ، لہذا اس پرچم کو چھوڑ دیں
  اگر آپ کو `plan chunk count` غلطیوں کا سامنا کرنا پڑتا ہے تو غیر فعال کریں اور دستی طور پر چیک کریں
  ایک بار جب گیٹ اوپر کی طرف جاتا ہے۔
- `--sign` - `sorafs_cli manifest sign` کی درخواست کرتا ہے۔ ایک ٹوکن کے ذریعے فراہم کریں
  `SIGSTORE_ID_TOKEN` (یا `--sigstore-token-env`) یا سی ایل آئی کو اس کے ذریعے بازیافت کرنے دیں
  `--sigstore-provider/--sigstore-audience`۔

پروڈکشن نمونے کے ل i ، `docs/portal/scripts/sorafs-pin-release.sh` استعمال کریں۔
اب یہ پورٹل ، OpenAPI اور SBOM پیکج کرتا ہے ، ہر مینی فیسٹ پر دستخط کرتا ہے ، اور ریکارڈز
`portal.additional_assets.json` میں اضافی میٹا ڈیٹا اثاثے۔ مددگار
اسی اختیاری نوبس پر مشتمل ہے جیسے سی آئی پیکجر کے علاوہ نئے سوئچز
`--openapi-*` ، `--portal-sbom-*` ، اور `--openapi-sbom-*` عرف ٹوپل کو تفویض کرنے کے لئے
نمونے کے ذریعہ ، `--openapi-sbom-source` کے ذریعے SBOM ماخذ کی اوور رائڈ ، کچھ چھوڑ دیں
پے لوڈ (`--skip-openapi`/`--skip-sbom`) ، اور بائنری `syft` غیر ڈیفالٹ کی طرف اشارہ کریں
`--syft-bin` کے ساتھ۔

اسکرپٹ میں ہر کمانڈ کو پھانسی دی گئی دکھائی دیتی ہے۔ لاگ کو ریلیز ٹکٹ میں کاپی کریں
`package_summary.json` کے ساتھ تاکہ جائزہ لینے والے کار ڈائجسٹ کا موازنہ کرسکیں ،
پلان میٹا ڈیٹا ، اور ایڈ ہاک شیل آؤٹ پٹ کو براؤز کیے بغیر Sigstore بنڈل کی ہیشیں۔

## مرحلہ 9 - توثیق گیٹ وے + سورادنس

کٹ اوور کا اعلان کرنے سے پہلے ، یہ ثابت کریں کہ نیا عرف سوردنز کے ذریعہ حل کرتا ہے اور گیٹ وے
بنیادی ثبوت:

1. ** گیٹ کی تحقیقات چلائیں۔ ** `ci/check_sorafs_gateway_probe.sh` مشقیں
   `cargo xtask sorafs-gateway-probe` میں ڈیمو فکسچر کے خلاف
   `fixtures/sorafs_gateway/probe_demo/`۔ حقیقی تعیناتیوں کے لئے ، تحقیقات کی نشاندہی کریں
   ہدف کے میزبان نام کے لئے:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   تحقیقات `Sora-Name` ، `Sora-Proof` ، اور `Sora-Proof-Status` کے مطابق
   `docs/source/sorafs_alias_policy.md` اور جب ہضم ظاہر ہوتا ہے تو ناکام ہوجاتا ہے ،
   ٹی ٹی ایل یا گار پابندیاں اخذ کرتی ہیں۔ہلکا پھلکا اسپاٹ چیک کے لئے (مثال کے طور پر ، جب صرف پابند بنڈل
   تبدیل شدہ) ، `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>` چلائیں۔
   مددگار پکڑے گئے بائنڈنگ بنڈل کی توثیق کرتا ہے اور رہائی کے لئے آسان ہے
   وہ ٹکٹ جن کو صرف مکمل تحقیقات کی مشق کی بجائے پابند تصدیق کی ضرورت ہوتی ہے۔

2. ** ڈرل کے ثبوت پر قبضہ کریں۔
   تحقیقات کو `اسکرپٹ/ٹیلی میٹری/رن_سورافس_گیٹ وے_پروبی.ش -اسکینریو کے ساتھ لپیٹیں
   devuporal-rollout-... `. ریپر ہیڈر/لاگ ان کے تحت اسٹور کرتا ہے
   `artifacts/sorafs_gateway_probe/<stamp>/` ، تازہ ترین `ops/drill-log.md` ، اور
   (اختیاری طور پر) رول بیک ہکس یا پیجریڈی پے لوڈ کو متحرک کرتا ہے۔ وضاحت کریں
   `--host docs.sora` سخت کوڈڈ IP کے بجائے سورادنس کے راستے کو درست کرنے کے لئے۔

3. ** ڈی این ایس بائنڈنگز چیک کریں۔
   تحقیقات (`--gar`) کے ذریعہ GAR فائل کا حوالہ دیا گیا ہے اور اسے ریلیز شواہد کے ساتھ منسلک کریں۔
   مالکان حل کرنے والا ایک ہی ان پٹ کو `tools/soradns-resolver` کے ذریعے دوبارہ چلا سکتا ہے
   اس بات کو یقینی بنائیں کہ کیچڈ اندراجات نئے منشور کا احترام کریں۔ JSON منسلک کرنے سے پہلے ،
   عملدرآمد
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   آف لائن آف لائن ڈٹرمینسٹک میزبان نقشہ سازی ، مینی فیسٹ میٹا ڈیٹا اور لیبلوں کی توثیق کرنے کے لئے
   ٹیلی میٹری۔ مددگار ایک سمری `--json-out` جاری کرسکتا ہے جس کے ساتھ GAR پر دستخط ہوئے ہیں
   جائزہ لینے والوں کے پاس بائنری کھولے بغیر تصدیق کے ثبوت موجود ہیں۔
  نیا گار لکھتے وقت ، ترجیح دیں
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (`--manifest-cid <cid>` میں صرف اس وقت جب مینی فیسٹ فائل نہ ہو
  دستیاب)۔ مددگار اب سی آئی ڈی ** اور ** بلیک 3 ڈائجسٹ کو براہ راست اخذ کرتا ہے
  JSON منشور سے ، خالی جگہوں کو کاٹیں ، جھنڈوں `--telemetry-label` کو نقل کریں ،
  لیبلوں کو ترتیب دیتا ہے ، اور پہلے سے طے شدہ CSP/HSTS/اجازت-پالیسی ٹیمپلیٹس کو خارج کرتا ہے
  JSON لکھنے کے ل to تاکہ پے لوڈ کا بوجھ نہ ہو یہاں تک کہ اگر آپریٹرز
  مختلف گولوں سے لیبل پر قبضہ کریں۔

4. ** عرفی میٹرکس کی نگرانی کریں۔ ** `torii_sorafs_alias_cache_refresh_duration_ms` رکھیں
   اور تحقیقات کے دوران اسکرین پر `torii_sorafs_gateway_refusals_total{profile="docs"}` ؛
   یہ سلسلہ `dashboards/grafana/docs_portal.json` میں گرافڈ ہے۔

## مرحلہ 10 - نگرانی اور ثبوت کا بنڈل- ** ڈیش بورڈز۔
  `dashboards/grafana/sorafs_gateway_observability.json` (گیٹ وے لیٹینسی +
  صحت کا ثبوت) ، اور `dashboards/grafana/sorafs_fetch_observability.json`
  (ہیلتھ آرکیسٹریٹر) ہر ریلیز کے لئے۔ JSON برآمدات کو رہائی کے ٹکٹ سے منسلک کریں
  تاکہ جائزہ لینے والے سوالات کو دوبارہ چلا سکتے ہیں Prometheus۔
- ** تحقیقات آرکائیوز ** `artifacts/sorafs_gateway_probe/<stamp>/` کو گٹ-اینیکس میں رکھیں
  یا آپ کی ثبوت کی بالٹی۔ دوبارہ شروع کی تحقیقات ، ہیڈر ، اور پیجریڈی پے لوڈ کو شامل کریں
  ٹیلی میٹری اسکرپٹ کے ذریعہ پکڑا گیا۔
- ** ریلیز بنڈل۔
  دستخط Sigstore ، `portal.pin.report.json` ، TRY-IT تحقیقات لاگز ، اور لنک چیک رپورٹس
  ٹائم اسٹیمپ فولڈر کے تحت (مثال کے طور پر ، `artifacts/sorafs/devportal/20260212T1103Z/`)۔
- ** ڈرل لاگ۔ ** جب تحقیقات ڈرل کا حصہ ہوں تو چھوڑ دیں
  `scripts/telemetry/run_sorafs_gateway_probe.sh` `ops/drill-log.md` میں اندراجات شامل کریں
  تاکہ وہی ثبوت SNNET-5 افراتفری کی ضرورت کو پورا کریں۔
- ** ٹکٹ کے لنکس۔ ** پینل IDS Grafana یا منسلک PNG برآمدات میں حوالہ دیں
  تحقیقات کی رپورٹ کے راستے سے ٹکٹ تبدیل کریں ، تاکہ جائزہ لینے والے کر سکیں
  شیل تک رسائی کے بغیر کراس ریفرنس سلوس۔

## مرحلہ 11 - ملٹی سورس اور ثبوت اسکور بورڈ کو ڈرل کریں

SoraFS پر اشاعت کے لئے اب ملٹی سورس ثبوت بازیافت کی ضرورت ہے (DOCS-7/SF-6)
مذکورہ بالا DNS/گیٹ وے ثبوت کے ساتھ۔ ظاہر پن کے بعد:

1. ** براہ راست منشور کے خلاف `sorafs_fetch` چلائیں۔ ** استعمال کریں منصوبہ/ظاہر نمونے
   ہر فراہم کنندہ کے لئے جاری کردہ گیٹ وے کی اسناد کے ساتھ 2-3 مرحلے میں مصنوعات۔
   تمام نتائج کو برقرار رکھیں تاکہ سامعین فیصلے کا سراغ لگاسکیں
   آرکسٹریٹر:

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

   - پہلے اشتہارات فراہم کرنے والوں کے حوالہ جات کو منشور سے بازیافت کریں (مثال کے طور پر
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     اور ان کو `--provider-advert name=path` کے ذریعے پاس کریں تاکہ اسکور بورڈ اس کا اندازہ کرے
     صلاحیت کے ونڈوز ایک عصبی انداز میں۔ استعمال کریں
     `--allow-implicit-provider-metadata` ** صرف ** جب CI میں فکسچر کو دوبارہ چلاتے ہو ؛
     پروڈکشن ڈرلوں کو انتباہی علامات کا حوالہ دینا ہوگا جو پن کے ساتھ تھے۔
   - جب ظاہر ہوتا ہے اضافی خطوں کا حوالہ دیتے ہیں تو ، کمانڈ کو اس کے ساتھ دہرائیں
     فراہم کنندہ کے ساتھ ملاپ کرنے والا تاکہ ہر کیشے/عرف سے وابستہ بازیافت کا نمونہ ہو۔

2. ** محفوظ شدہ دستاویزات۔ ** `scoreboard.json` رکھیں ،
   `providers.ndjson` ، `fetch.json` ، اور `chunk_receipts.ndjson` ثبوت فولڈر کے تحت
   رہائی کی یہ فائلیں وزن والے ساتھیوں ، دوبارہ کوشش کا بجٹ ، EWMA لیٹینسی پر قبضہ کرتی ہیں
   اور وہ رسیدیں جو گورننس پیکیج کو SF-7 کے لئے یاد رکھنا چاہئے۔

3. ** ٹیلی میٹری کو اپ ڈیٹ کریں۔ ** ڈیش بورڈ میں بازیافت کے آؤٹ پٹ کو درآمد کریں
   ** SoraFS بازیافت کی بازیافت ** (`dashboards/grafana/sorafs_fetch_observability.json`) ،
   `torii_sorafs_fetch_duration_ms`/`_failures_total` اور رینج فراہم کرنے والے پینل کی نگرانی کریں
   بے ضابطگیوں کے لئے۔ اسنیپ شاٹس Grafana کو ریلیز ٹکٹ سے اسکور بورڈ کے راستے سے لنک کریں۔4. ** ٹیسٹ الرٹ کے قواعد۔ ** `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں
   رہائی بند کرنے سے پہلے الرٹ بنڈل Prometheus کی توثیق کرنے کے لئے۔ آؤٹ پٹ میں شامل ہوں
   اس بات کی تصدیق کے ل docs دستاویزات -7 جائزہ لینے والوں کے لئے ٹکٹ پر پرومٹول
   سست فراہم کرنے والا مسلح رہتا ہے۔

5. ** سی آئی میں پلگ ان۔
   اندراج `perform_fetch_probe` ؛ اسٹیجنگ/پروڈکشن رنز کے ل it اسے قابل بنائیں تاکہ
   ثبوت بازیافت دستی مداخلت کے بغیر مینی فیسٹ بنڈل کے ساتھ تیار کی جاتی ہے۔
   مقامی مشقیں گیٹ وے ٹوکن اور برآمد کرکے اسی اسکرپٹ کو دوبارہ استعمال کرسکتی ہیں
   `PIN_FETCH_PROVIDERS` کو فراہم کرنے والوں کی کوما سے الگ فہرست میں ترتیب دے کر۔

## تشہیر ، مشاہدہ اور رول بیک

1. ** پروموشن: ** اسٹیجنگ اور پروڈکشن کے لئے الگ الگ عرفیت رکھیں۔ میں فروغ دیں
   اسی مینی فیسٹ/بنڈل کے ساتھ `manifest submit` کو دوبارہ چلائیں ، تبدیل ہو رہے ہیں
   `--alias-namespace/--alias-name` پروڈکشن عرف کی طرف اشارہ کرنے کے لئے۔ یہ گریز کرتا ہے
   ایک بار جب کیو اے نے پن اسٹیجنگ کی منظوری دے دی تو دوبارہ تعمیر یا دوبارہ دستخط کرنا۔
2. ** نگرانی: ** پن رجسٹری ڈیش بورڈ درآمد کریں
   (`docs/source/grafana_sorafs_pin_registry.json`) پلس مخصوص گیٹ پروبس
   (`docs/portal/docs/devportal/observability.md` دیکھیں)۔ چیکسم بڑھے ہوئے انتباہ ،
   ناکام تحقیقات یا دوبارہ پروف پروف اسپائکس۔
3. ** رول بیک: ** واپس جانے کے لئے ، پچھلے مینی فیسٹ (یا ہٹائیں
   `sorafs_cli manifest submit --alias ... --retire` کے ساتھ موجودہ عرف)۔
   آخری بنڈل کو ہمیشہ اچھ and ا اور دوبارہ شروع کی کار کو رکھیں تاکہ
   اگر CI لاگز چل رہے ہیں تو رول بیک ثبوتوں کو دوبارہ سے تیار کیا جاسکتا ہے۔

## CI ورک فلو ماڈل

کم سے کم ، آپ کی پائپ لائن کو چاہئے:

1. بلڈ + لنٹ (`npm ci` ، `npm run build` ، چیکس کی نسل)۔
2. پیکیج (`car pack`) اور منشور کا حساب لگائیں۔
3. نوکری ٹوکن OIDC (`manifest sign`) کا استعمال کرتے ہوئے دستخط کریں۔
4. آڈٹ کے لئے نمونے (کار ، منشور ، بنڈل ، منصوبہ ، خلاصہ) اپ لوڈ کریں۔
5. پن رجسٹری میں جمع کروائیں:
   - درخواستیں کھینچیں -> `docs-preview.sora`۔
   - محفوظ ٹیگز / شاخیں -> پروموشن عرف کی پیداوار۔
6. فائننگ سے پہلے پروف تصدیق کی تحقیقات + گیٹس چلائیں۔

`.github/workflows/docs-portal-sorafs-pin.yml` دستی ریلیز کے ل these ان تمام مراحل کو جوڑتا ہے۔
ورک فلو:

- پورٹل کی تعمیر/جانچ ،
- `scripts/sorafs-pin-release.sh` کے ذریعے تعمیر کے پیکیجز ،
- Github OIDC کے توسط سے مینی فیسٹ بنڈل پر دستخط/تصدیق کریں ،
- کار/منشور/بنڈل/پلان/پلان/ریزیومز کو نمونے کے طور پر اپ لوڈ کریں ، اور
- (اختیاری طور پر) جب راز موجود ہوں تو مینی فیسٹ + عرف بائنڈنگ پیش کرتا ہے۔

نوکری کو متحرک کرنے سے پہلے درج ذیل راز/متغیرات کی تشکیل کریں:| نام | مقصد |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | میزبان Torii جو `/v1/sorafs/pin/register` کو بے نقاب کرتا ہے۔ |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | گذارشات کے ساتھ ایپچ ID محفوظ کیا گیا۔ |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | ظاہر جمع کرانے کے لئے اتھارٹی پر دستخط کرنا۔ |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | جب `perform_submit` `true` ہے تو ٹپل الیاس ظاہر ہونے کا پابند ہے۔ |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | بنڈل پروف الیاس بیس 64 انکوڈس (اختیاری ؛ عرف بائنڈنگ کو چھوڑنے کے لئے چھوڑ دیں)۔ |
| `DOCS_ANALYTICS_*` | موجودہ تجزیات/تحقیقات کے اختتامی مقامات دوسرے ورک فلوز کے ذریعہ دوبارہ استعمال کیے جاتے ہیں۔ |

کاموں کے ذریعے ورک فلو کو متحرک کریں UI:

1. `alias_label` فراہم کریں (مثال کے طور پر: `docs.sora.link`) ، ایک اختیاری `proposal_alias` ،
   اور ایک اختیاری `release_tag` اوور رائڈ۔
2. چھوڑو `perform_submit` Torii کو چھوئے بغیر نمونے تیار کرنے کے لئے بغیر چیک کیا
   ۔

`docs/source/sorafs_ci_templates.md` مزید دستاویزات عام سی آئی مددگاروں کے لئے
غیر ریپو پروجیکٹس ، لیکن پورٹل ورک فلو ترجیحی راستہ ہونا چاہئے
روزانہ ریلیز کے لئے۔

## چیک لسٹ

- [] `npm run build` ، `npm run test:*` ، اور `npm run check:links` سبز ہیں۔
- [] `build/checksums.sha256` اور `build/release.json` نمونے میں گرفتاری۔
- [] کار ، منصوبہ ، منشور ، اور خلاصہ `artifacts/` کے تحت تیار کیا گیا ہے۔
- [] بنڈل Sigstore + نوشتہ جات کے ساتھ ذخیرہ شدہ دستخط۔
- [] `portal.manifest.submit.summary.json` اور `portal.manifest.submit.response.json`
      جب گذارشات ہوتی ہیں تو گرفتاری۔
- [] `portal.pin.report.json` (اور اختیاری `portal.pin.proposal.json`)
      کار/منشور نمونے کے ساتھ محفوظ شدہ دستاویزات۔
- [] لاگ ان `proof verify` اور `manifest verify-signature` آرکائیوز۔
- [] ڈیش بورڈز Grafana تازہ کاری + TRY-IT تحقیقات کامیاب۔
- [] رول بیک نوٹ (پچھلا مینی فیسٹ ID + ڈائجسٹ عرف) سے منسلک
      ٹکٹ کی رہائی