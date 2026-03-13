---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## جائزہ

یہ طریقہ کار گائیڈ ** دستاویزات -7 ** (اشاعت SoraFS) اور ** دستاویزات -8 ** (CI/CD میں پن آٹومیشن) روڈ میپ عناصر کو عملی ڈویلپر پورٹل طریقہ کار میں تبدیل کرتا ہے۔ اس میں بلڈ/لنٹ مرحلے کا احاطہ کیا گیا ہے ، SoraFS کو لپیٹ کر ، Sigstore ، عرف اپ گریڈ ، توثیق ، ​​اور رول بیک مشقوں کے ذریعہ مینی فیسٹ پر دستخط کریں تاکہ ہر پیش نظارہ اور رہائی قابل تولیدی اور قابل آیت ہو۔

اس راستے سے یہ فرض کیا گیا ہے کہ آپ کے پاس `sorafs_cli` بائنری ہے (`--features cli` کے ذریعے بنایا گیا ہے) ، Torii اختتامی نقطہ تک PIN-Registry مراعات کے ساتھ رسائی ، اور OIDC اسناد Sigstore کے لئے ہے۔ سی آئی والٹ میں طویل عرصے تک راز (`IROHA_PRIVATE_KEY` ، `SIGSTORE_ID_TOKEN` ، کوڈ Torii) اسٹور کریں۔ مقامی عمل انہیں شیل سے برآمدات کے ذریعے لوڈ کرسکتے ہیں۔

## شرائط

- نوڈ 18.18 یا بعد میں `npm` یا `pnpm` کے ساتھ۔
- `sorafs_cli` `cargo run -p sorafs_car --features cli --bin sorafs_cli` سے۔
- ایڈریس Torii `/v2/sorafs/*` کو کسی اتھارٹی کے نجی اکاؤنٹ/کلید کے ساتھ ظاہر کرتا ہے جو ظاہر اور عرفی نام بھیج سکتا ہے۔
- `SIGSTORE_ID_TOKEN` ریلیز کے لئے OIDC ایکسپورٹر (گٹ ہب ایکشنز ، گٹ لیب ، ورک بوجھ کی شناخت ، وغیرہ)۔
- اختیاری: خشک تجربات کے لئے `examples/sorafs_cli_quickstart.sh` اور Github/Gitlab میں ورک فلو ٹیمپلیٹس کے لئے `docs/source/sorafs_ci_templates.md`۔
- کوشش کرنے کے لئے oauth متغیرات مرتب کریں (`DOCS_OAUTH_*`) اور چلائیں
  [سیکیورٹی سخت کرنے والی چیک لسٹ] (./security-hardening.md) لیب سے باہر کی تعمیر کو اپ گریڈ کرنے سے پہلے۔ گیٹ وے کی تعمیر اب اس وقت ناکام ہوجاتی ہے جب یہ متغیر غیر حاضر ہوں یا جب ٹی ٹی ایل/پولنگ کیز مسلط ونڈوز سے باہر جاتے ہیں۔ صرف عارضی مقامی پیش نظاروں کے لئے `DOCS_OAUTH_ALLOW_INSECURE=1` استعمال کریں۔ رہائی کے ٹکٹ سے دخول ٹیسٹ کے ثبوت منسلک کریں۔

## مرحلہ 0 - پراکسی پیکٹ کیپچر اس کی کوشش کریں

پیش نظارہ کو نیٹ لائف یا گیٹ وے میں اپ گریڈ کرنے سے پہلے ، کوشش کریں کہ آئی ٹی ایجنٹ کے ذرائع کو انسٹال کریں اور دستخط شدہ فنگر پرنٹ OpenAPI کو ایک تعی .ن پیکیج میں:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` کاپکس پراکسی/تحقیقات/رول بیک ، OpenAPI کے دستخط کی تصدیق کرتا ہے اور `release.json` کو `checksums.sha256` کے ساتھ لکھتا ہے۔ اس پیکیج کو نیٹ لیف/SoraFS گیٹ وے اپ گریڈ ٹکٹ کے ساتھ شامل کیا گیا ہے تاکہ جائزہ لینے والے قطعی پراکسی وسائل اور Torii اشارے کو دوبارہ تعمیر کے بغیر دوبارہ چلاسکیں۔ اس پیکیج میں یہ بھی لاگ ان ہوتا ہے کہ آیا کلائنٹ (`allow_client_auth`) کے ذریعہ بھیجے گئے بیئرر ٹوکن لانچ پلان اور سی ایس پی کے قواعد کو مستقل رکھنے کے قابل ہیں۔

## مرحلہ 1 - گیٹ وے کے لئے لنٹ کی تعمیر اور معائنہ کریں

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

`npm run build` خود بخود `scripts/write-checksums.mjs` پر عملدرآمد کرتا ہے اور تیار کرتا ہے:

- `build/checksums.sha256` - `sha256sum -c` کے لئے درست SHA256 منشور۔
- `build/release.json` - میٹا ڈیٹا (`tag` ، `generated_at` ، `source`) ہر کار/مینی فیسٹ میں نصب ہے۔

دونوں فائلوں کو کار سمری کے ساتھ محفوظ کریں تاکہ جائزہ لینے والے بغیر کسی تعمیر نو کے معائنہ کے نتائج کا موازنہ کرسکیں۔

## مرحلہ 2 - پیکیجنگ فکسڈ اثاثے

Docusaurus آؤٹ پٹ کے خلاف کار پیکر چلائیں۔ ذیل کی مثال `artifacts/devportal/` کے تحت تمام اثاثوں کو لکھتی ہے۔

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

JSON کا خلاصہ ٹکڑوں ، ہضموں اور پروف ترتیب کے اشارے کی تعداد حاصل کرتا ہے جو `manifest build` اور CI بورڈز کے بعد دوبارہ استعمال کرتے ہیں۔

## مرحلہ 2B - OpenAPI اور SBOM CHAPERONES کی encapsulationDOCS-7 کے لئے پورٹل سائٹ ، اسنیپ شاٹ OpenAPI ، اور SBOM کو الگ الگ منشور کے طور پر شائع کرنے کی ضرورت ہے تاکہ پورٹل `Sora-Proof`/`Sora-Content-CID` ہیڈر کو ہر نمونے پر رکھ سکے۔ ریلیز اسسٹنٹ (`scripts/sorafs-pin-release.sh`) دراصل OpenAPI فولڈر (`static/openapi/`) اور نتیجے میں SBOM فائلوں کو `syft` کے ذریعے الگ الگ کاروں `openapi.*`/`*-sbom.*` اور ریکارڈز میں شامل کرتا ہے۔ جب دستی راستے پر عمل پیرا ہو تو ، ہر پے لوڈ کے لئے اس کے سابقہ ​​اور میٹا ڈیٹا ٹیگز (جیسے `--car-out "$OUT"/openapi.car` کے ساتھ `--metadata alias_label=docs.sora.link/openapi`) کے ساتھ 2-4 اقدامات دہرائیں۔ ڈی این ایس سوئچ سے پہلے ہر مینی فیسٹ/عرف جوڑی کو Torii (مقام ، OpenAPI ، گیٹ وے SBOM ، SBOM OpenAPI) پر رجسٹر کریں تاکہ گیٹ وے ہر پوسٹ کردہ نوادرات کے لئے پنڈڈ ثبوت فراہم کرسکے۔

## مرحلہ 3 - منشور کی تعمیر

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

ورژن ونڈو کے مطابق پن پالیسی کے جھنڈوں میں ترمیم کریں (مثال کے طور پر ، کینری کے لئے `--pin-storage-class hot`)۔ JSON ورژن اختیاری ہے لیکن جائزہ لینے کے لئے مفید ہے۔

## مرحلہ 4 - Sigstore کے ذریعے سائن کریں

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

بنڈل نے جے ڈبلیو ٹی کو بچانے کے بغیر OIDC ٹوکن کے مینی فیسٹ کے فنگر پرنٹ ، ٹکڑوں کا فنگر پرنٹ ، اور بلیک 3 ہیش ریکارڈ کیا ہے۔ بنڈل اور دستخط کو الگ رکھیں۔ پروڈکشن اپ گریڈ ایک ہی اثاثوں کو دوبارہ دستخط کیے بغیر دوبارہ استعمال کرسکتے ہیں۔ جب بیرونی Sigstore مددگار ہائیپر ٹوکن جاری کرتا ہے تو مقامی عمل فراہم کنندہ کے جھنڈے کو `--identity-token-env` (یا ماحول میں `SIGSTORE_ID_TOKEN` سیٹ کریں) کے ساتھ تبدیل کرسکتے ہیں۔

## مرحلہ 5 - پن رجسٹری کو بھیجنا

Torii پر دستخط شدہ مینی فیسٹ (اور ٹکڑوں کا منصوبہ) بھیجیں۔ ہمیشہ ایک خلاصہ کی درخواست کریں تاکہ رجسٹریشن/عرف کا نتیجہ قابل آیت ہو۔

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

جب پیش نظارہ یا کینری (`docs-preview.sora`) جاری کرتے ہو تو ، ایک مختلف عرف کا استعمال کرتے ہوئے دوبارہ جمع کروائیں تاکہ QA پروڈکشن اپ گریڈ سے پہلے تصدیق کرسکے۔

عرف بائنڈنگ کے لئے تین فیلڈز کی ضرورت ہے: `--alias-namespace` ، `--alias-name` ، اور `--alias-proof`۔ گورننس ٹیم منظوری کے بعد بنڈل پروف (BASE64 یا Norito بائٹس) تیار کرتی ہے۔ اسے CI کے راز کے طور پر اسٹور کریں اور `manifest submit` پر کال کرنے سے پہلے اسے فائل کے طور پر پیش کریں۔ اگر آپ DNS کو تبدیل کیے بغیر منشور انسٹال کرنا چاہتے ہیں تو عرف پرچموں کو خالی چھوڑ دیں۔

## مرحلہ 5B - گورننس کی تجویز تیار کریں

ہر منشور کے ساتھ پارلیمنٹ کے لئے تیار تجویز بھی ہونی چاہئے تاکہ کوئی بھی سورہ شہری مراعات یافتہ اسناد کے بغیر تبدیلی پیش کرسکے۔ جمع کرانے/سائن اقدامات کے بعد ، عمل کریں:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` معیاری ہدایت `RegisterPinManifest` ، حصوں کے فنگر پرنٹ ، پالیسی ، اور عرف اشارے پر قبضہ کرتا ہے۔ اسے گورننس ٹکٹ یا پارلیمنٹ کے پورٹل سے منسلک کریں تاکہ مندوبین بغیر کسی تعمیر نو کے پے لوڈ کا جائزہ لے سکیں۔ یہ کمانڈ Torii کلید کو نہیں چھوتا ہے ، لہذا کوئی بھی شہری مقامی طور پر اس تجویز کو تشکیل دے سکتا ہے۔

## مرحلہ 6 - ثبوت اور پیمائش چیک کریں

تنصیب کے بعد ، لازمی توثیق کے اقدامات انجام دیں:

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

- عدم استحکام کا پتہ لگانے کے لئے `torii_sorafs_gateway_refusals_total` اور `torii_sorafs_replication_sla_total{outcome="missed"}` کی نگرانی کریں۔
- انسٹال کردہ مواد پر TRY-IT ایجنٹ اور توثیق کے لنکس کو جانچنے کے لئے `npm run probe:portal` چلائیں۔
- جس نگرانی کے ثبوت میں ذکر کیا گیا ہے اسے جمع کریں
  [پبلشنگ اینڈ مانیٹرنگ] (./publishing-monitoring.md) دستاویزات -3 سی آبزرویشن گیٹ کو پورا کرنے کے لئے۔ ہیلپر اب متعدد `bindings` (مقام ، OpenAPI ، گیٹ وے کے لئے SBOM ، OpenAPI کے لئے SBOM) اور فورسز `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` کو ایک سینٹینل کے ذریعے میزبان پر سپورٹ کرتا ہے۔ `hostname`۔ نیچے دیئے گئے کال میں ایک سنگل JSON DIST اور ڈائریکٹریوں کا پیکیج لکھتا ہے (`portal.json` ، `tryit.json` ، `binding.json` ، `checksums.sha256`) ریلیز فولڈر کے تحت:```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## مرحلہ 6 اے - پورٹل سرٹیفکیٹ کی منصوبہ بندی

GAR پیکٹ تیار کرنے سے پہلے TLS SAN/چیلنج پلان نکالیں تاکہ گیٹ وے ٹیم اور DNS کی منظوری اسی ثبوتوں کا جائزہ لیں۔ نیا مددگار ڈی جی 3 کے ان پٹ کو وائلڈ کارڈ لیگل ہوسٹ انیمریشن ، خوبصورت میزبان سان ، ڈی این ایس -01 لیبلوں ، اور تجویز کردہ چیلنجوں کے ذریعے ظاہر کرتا ہے۔

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON کو ریلیز پیکیج کے ساتھ شامل کریں (یا اس کے ساتھ تبدیلی کے ٹکٹ کے ساتھ) تاکہ آپریٹرز SAN اقدار کو `torii.sorafs_gateway.acme` ترتیبات میں چسپاں کرسکیں اور GAR جائزہ لینے والے بغیر کسی بازیافت کے کیننیکل/خوبصورت نقشوں کی جانچ کرسکتے ہیں۔ اسی ریلیز میں اپ گریڈ ہونے والے ہر اضافی لاحقہ کے لئے `--name` شامل کریں۔

## مرحلہ 6B - قانونی میزبان کے نقشے اخذ کریں

GAR ٹیمپلیٹس تیار کرنے سے پہلے ، ہر عرف کے لئے ڈٹرمینسٹک میزبان نقشہ ریکارڈ کریں۔ `cargo xtask soradns-hosts` ہر ایک `--name` کو اس کے قانونی عہدہ (`<base32>.gw.sora.id`) تک شکل دیتا ہے ، درخواست کردہ وائلڈ کارڈ (`*.gw.sora.id`) کو جاری کرتا ہے ، اور پیاری (`<alias>.gw.sora.name`) اخذ کرتا ہے۔ نوادرات کے ورژن میں آؤٹ پٹ کو محفوظ کریں تاکہ DG-3 جائزہ لینے والے نقشہ کو GAR جمع کرانے کے ساتھ موازنہ کرسکیں:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

تیز فیل اوور کے لئے `--verify-host-patterns <file>` استعمال کریں جب مطلوبہ میزبانوں میں سے ایک بائنڈنگ GAR یا JSON سے غائب ہے۔ مددگار توثیق کے ل multiple متعدد فائلوں کو قبول کرتا ہے ، جس سے اسی کال میں گار ٹیمپلیٹ اور `portal.gateway.binding.json` کا آڈٹ کرنا آسان ہوجاتا ہے:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

ڈی این ایس/گیٹ وے تبدیلی کے ٹکٹ پر JSON کا خلاصہ اور توثیق لاگ منسلک کریں تاکہ آڈیٹر دوبارہ چلانے والی اسکرپٹ کے بغیر جائز/اچھے میزبانوں کی تصدیق کرسکیں۔ نئے عرف کو شامل کرتے وقت کمانڈ کو دوبارہ چلائیں تاکہ GAR کی تازہ کارییں ایک ہی ڈائریکٹریوں کا وارث ہوں۔

## مرحلہ 7 - ڈی این ایس کٹ اوور ڈسکرپٹر تیار کریں

پیداوار میں کٹوتیوں کے لئے ایک آڈٹیبل چینج پیکیج کی ضرورت ہوتی ہے۔ کامیاب ٹرانسمیشن (عرف کو جوڑنے) کے بعد ، اسسٹنٹ ایشوز
`artifacts/sorafs/portal.dns-cutover.json` ، جس میں شامل ہیں:

- عرف بائنڈنگ ڈیٹا (نام کی جگہ/نام/ثبوت ، فنگر پرنٹ ، منشور ، ایڈریس Torii ، ایپچ ٹرانسمیٹر ، اتھارٹی)۔
- ریلیز سیاق و سباق (ٹیگ ، عرف لیبل ، منشور/کار کے راستے ، ٹکڑوں کا منصوبہ ، Sigstore بنڈل)۔
- توثیق کے اشارے (تحقیقات کمانڈ ، عرف + Torii اختتامی نقطہ)۔
- اختیاری تبدیلی والے فیلڈز (ٹکٹ ID ، کٹ اوور ونڈو ، آپریشنل رابطہ ، پروڈکشن میزبان/زون)۔
- `Sora-Route-Binding` ہیڈر (قانونی میزبان/سی آئی ڈی ، ہیڈر + بائنڈنگ راہیں ، توثیق کے کمانڈز) سے اخذ کردہ پاتھ اپ گریڈ ڈیٹا کو اس بات کا یقین کرنے کے لئے کہ GAR اپ گریڈ اور رول بیک مشقیں اسی ڈائریکٹریوں کی طرف اشارہ کرتی ہیں۔
-تیار کردہ روٹ پلان فائلیں (`gateway.route_plan.json` ، ہیڈر ٹیمپلیٹس ، اور اختیاری رول بیک ہیڈر) تاکہ CI میں DG-3 اور لنٹ ٹکٹ اپ گریڈ/رول بیک منصوبوں کی جانچ کریں۔
- کیشے کو منسوخ کرنے کے لئے اختیاری اعداد و شمار (پرج پوائنٹ ، آتھ متغیر ، JSON پے لوڈ ، مثال `curl`)۔
- کالعدم کے ناگزیر راستے کو برقرار رکھنے کے لئے پچھلے تفصیل (ایشو ٹیگ اور منشور فنگر پرنٹ) کا حوالہ دینے والے اشارے کو کالعدم کریں۔

جب صاف کرنے کی کارروائیوں کی ضرورت ہو تو ، تفصیل کے ساتھ قانونی منصوبہ تیار کریں:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

`portal.cache_plan.json` کو DG-3 پیکیج میں شامل کیا گیا ہے تاکہ آپریٹرز کو `PURGE` درخواستیں بھیجتے وقت ڈٹرمینسٹک راستے/میزبان (اور Auth فلٹرز) ہوں۔ تفصیل میں ایک اختیاری سیکشن براہ راست اس فائل کا حوالہ دے سکتا ہے۔

ڈی جی 3 پیکیج کو اپ گریڈ اور رول بیک چیک لسٹ کی بھی ضرورت ہے۔ اسے `cargo xtask soradns-route-plan` کے ذریعے تیار کریں:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` قانونی/خوبصورت میزبانوں ، عبوری صحت کی جانچ پڑتال کے لئے یاد دہانی ، GAR بائنڈنگ اپڈیٹس ، کیشے پرج ، اور رول بیک اقدامات کو ریکارڈ کرتا ہے۔ ٹکٹ بھیجنے سے پہلے اسے گار/بائنڈنگ/کٹ اوور فائلوں سے منسلک کریں تاکہ آپ کی او پی ایس ٹیم بھی اسی اقدامات پر عمل کرسکے۔`scripts/generate-dns-cutover-plan.mjs` خود بخود `sorafs-pin-release.sh` سے تفصیل تیار کرتا ہے۔ دستی نسل کے لئے:

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

مددگار چلانے سے پہلے ماحولیاتی متغیر کے ذریعہ اختیاری ڈیٹا کو پُر کریں:

| متغیر | مقصد |
| --- | --- |
| `DNS_CHANGE_TICKET` | تفصیل میں محفوظ ٹکٹ کی شناخت۔ |
| `DNS_CUTOVER_WINDOW` | ISO8601 فارمیٹ میں کٹ اوور ونڈو (جیسے `2026-03-21T15:00Z/2026-03-21T15:30Z`)۔ |
| `DNS_HOSTNAME` ، `DNS_ZONE` | پروڈکشن میزبان + خطے کی حمایت کی گئی۔ |
| `DNS_OPS_CONTACT` | ہنگامی رابطہ |
| `DNS_CACHE_PURGE_ENDPOINT` | تفصیل میں ذخیرہ شدہ پوائنٹ۔ |
| `DNS_CACHE_PURGE_AUTH_ENV` | ماحولیاتی متغیر ہولڈنگ ٹوکن پرج (پہلے سے طے شدہ `CACHE_PURGE_TOKEN`)۔ |
| `DNS_PREVIOUS_PLAN` | رجعت اعداد و شمار کی سابقہ ​​تفصیل کا راستہ۔ |

JSON کو DNS تبدیلی کے جائزے سے منسلک کریں تاکہ جائزہ لینے والے CI لاگز کی جانچ کیے بغیر ظاہر فنگر پرنٹ ، عرف میپنگ ، اور تحقیقات کے احکامات کی تصدیق کرسکیں۔ CLI جھنڈے فراہم کرتا ہے جیسے `--dns-change-ticket` ، `--dns-cutover-window` ، `--dns-hostname` ، `--dns-zone` ، `--ops-contact` ، `--cache-purge-endpoint` ، `--cache-purge-auth-env` ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```۔

## مرحلہ 8 - تجزیہ کار (اختیاری) کو زون فائل کا ڈھانچہ جاری کریں

جب پروڈکشن کٹ ونڈو کی وضاحت کی جاتی ہے تو ، ریلیز اسکرپٹ خود بخود ایس این ایس زون فائل ڈھانچے اور پارسر کے ٹکڑوں کو برآمد کرسکتا ہے۔ ماحولیاتی متغیرات یا سی ایل آئی کے اختیارات کے ذریعہ ڈی این ایس ریکارڈ اور ڈیٹا پاس کریں۔ مددگار تفصیل پیدا کرنے کے فورا بعد ہی `scripts/sns_zonefile_skeleton.py` پر کال کرے گا۔ کم از کم A/AAAA/CNAME ویلیو اور GAR فنگر پرنٹ (دستخط شدہ پے لوڈ کے لئے بلیک 3-256) فراہم کریں۔ اگر زون/میزبان جانا جاتا ہے اور `--dns-zonefile-out` حذف کردیا گیا ہے تو ، مددگار `artifacts/sns/zonefiles/<zone>/<hostname>.json` میں لکھے گا اور `ops/soradns/static_zones.<hostname>.json` کو حل کرنے والے اسنیپٹ کے طور پر تیار کرے گا۔

| متغیر/آپشن | مقصد |
| --- | --- |
| `DNS_ZONEFILE_OUT` ، `--dns-zonefile-out` | نتیجے میں زون فائل کنکال کا راستہ۔ |
| `DNS_ZONEFILE_RESOLVER_SNIPPET` ، `--dns-zonefile-resolver-snippet` | حل کرنے والا اسنیپٹ راہ (پہلے سے طے شدہ `ops/soradns/static_zones.<hostname>.json`)۔ |
| `DNS_ZONEFILE_TTL` ، `--dns-zonefile-ttl` | آؤٹ پٹ لاگز کے لئے ٹی ٹی ایل (پہلے سے طے شدہ: 600 سیکنڈ)۔ |
| `DNS_ZONEFILE_IPV4` ، `--dns-zonefile-ipv4` | IPv4 پتے (کوما یا سرخ پرچم سے الگ الگ فہرست)۔ |
| `DNS_ZONEFILE_IPV6` ، `--dns-zonefile-ipv6` | IPv6 پتے۔ |
| `DNS_ZONEFILE_CNAME` ، `--dns-zonefile-cname` | CNAME کا ہدف اختیاری ہے۔ |
| `DNS_ZONEFILE_SPKI` ، `--dns-zonefile-spki-pin` | SPKI SHA-256 (BASE64) فنگر پرنٹنگ۔ |
| `DNS_ZONEFILE_TXT` ، `--dns-zonefile-txt` | اضافی TXT ریکارڈز (`key=value`)۔ |
| `DNS_ZONEFILE_VERSION` ، `--dns-zonefile-version` | اوور رائڈ لیبل حساب کتاب ورژن۔ |
| `DNS_ZONEFILE_EFFECTIVE_AT` ، `--dns-zonefile-effective-at` | کلپنگ ونڈو کے آغاز کو تبدیل کرنے کے لئے ٹائم اسٹیمپ `effective_at` (RFC3339) کو مجبور کریں۔ |
| `DNS_ZONEFILE_PROOF` ، `--dns-zonefile-proof` | ڈیٹا میں رجسٹرڈ ثبوت کو نظرانداز کریں۔ |
| `DNS_ZONEFILE_CID` ، `--dns-zonefile-cid` | ڈیٹا میں رجسٹرڈ سی آئی ڈی کو اوور رائڈ کریں۔ |
| `DNS_ZONEFILE_FREEZE_STATE` ، `--dns-zonefile-freeze-state` | گارڈ جم جاتا ہے (نرم ، سخت ، پگھلنا ، نگرانی ، ہنگامی صورتحال)۔ |
| `DNS_ZONEFILE_FREEZE_TICKET` ، `--dns-zonefile-freeze-ticket` | ٹکٹ کا حوالہ منجمد کریں۔ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT` ، `--dns-zonefile-freeze-expires-at` | پگھلنے کے مرحلے کے لئے ٹائم اسٹیمپ آر ایف سی 33339۔ |
| `DNS_ZONEFILE_FREEZE_NOTES` ، `--dns-zonefile-freeze-note` | اضافی منجمد نوٹ (کوما سے الگ فہرست)۔ |
| `DNS_GAR_DIGEST` ، `--dns-gar-digest` | بلیک 3-256 (ہیکس) دستخط شدہ پے لوڈ کا فنگر پرنٹ۔ جب گیٹ وے کے لئے پابندیاں موجود ہوں تو ضروری ہے۔ |

گٹ ہب ایکشنز ان اقدار کو ذخیرہ راز سے پڑھتے ہیں تاکہ ہر پروڈکشن پن خود بخود نوادرات پیدا کرے۔ مندرجہ ذیل راز طے کریں (اقدار کوما سے الگ الگ فہرستوں پر مشتمل ہوسکتی ہے):| خفیہ | مقصد |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME` ، `DOCS_SORAFS_DNS_ZONE` | اسسٹنٹ کے لئے میزبان/پروڈکشن ایریا۔ |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | تفصیل میں ذخیرہ شدہ رابطہ کریں۔ |
| `DOCS_SORAFS_ZONEFILE_IPV4` ، `DOCS_SORAFS_ZONEFILE_IPV6` | تعیناتی کے لئے IPv4/IPv6 لاگ۔ |
| `DOCS_SORAFS_ZONEFILE_CNAME` | CNAME کا ہدف اختیاری ہے۔ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | SPKI BASE64 فنگر پرنٹس۔ |
| `DOCS_SORAFS_ZONEFILE_TXT` | اضافی TXT ریکارڈز۔ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | ڈھانچے میں اعداد و شمار کو منجمد کرنا۔ |
| `DOCS_SORAFS_GAR_DIGEST` | دستخط شدہ پے لوڈ کے ہیکس فارمیٹ میں بلیک 3 فنگر پرنٹ۔ |

جب `.github/workflows/docs-portal-sorafs-pin.yml` چلاتے ہو تو ، اندراجات `dns_change_ticket` اور `dns_cutover_window` فراہم کریں تاکہ تفصیل/زون فائل صحیح ٹرم ونڈو کو وراثت میں مل جائے۔ اسے صرف خشک رن آپریشنوں کے لئے خالی چھوڑ دیں۔

عام مثال (فی SN-7 مالک کی رن بک):

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

مددگار TXT اندراج کے طور پر خود بخود تبدیلی کا ٹکٹ کھینچتا ہے اور ٹرم ونڈو کے آغاز کو `effective_at` پر وراثت میں مل جاتا ہے جب تک کہ اس کو ختم نہ کیا جائے۔ مکمل آپریشنل راہ کے لئے `docs/source/sorafs_gateway_dns_owner_runbook.md` دیکھیں۔

### عالمی DNS وفد کے بارے میں ایک نوٹ

زون فائل ڈھانچہ صرف قابل اعتماد زون ریکارڈ کی وضاحت کرتا ہے۔ آپ کو ابھی بھی اپنے رجسٹرار یا DNS فراہم کنندہ کے ساتھ ہوم زون کے لئے NS/DS وفد مرتب کرنے کی ضرورت ہے تاکہ عوامی انٹرنیٹ آپ کے نام کے سرور تلاش کرسکے۔

- اپیکس/ٹی ایل ڈی میں کٹ اوورز کے لئے عرف/انیم (فراہم کنندہ پر منحصر ہے) استعمال کریں یا گیٹ وے کے کسی بھی کاسٹ ایڈریس کی طرف اشارہ کرتے ہوئے A/AAAA ریکارڈ شائع کریں۔
- سب ڈومینز کے لئے ، CNAME کو ماخوذ خوبصورت میزبان (`<fqdn>.gw.sora.name`) میں شائع کریں۔
- قانونی میزبان (`<hash>.gw.sora.id`) پورٹل کے ڈومین کے تحت رہتا ہے اور آپ کے عوامی ڈومین میں شائع نہیں ہوتا ہے۔

### گیٹ ہیڈر ٹیمپلیٹ

تعیناتی اسسٹنٹ `portal.gateway.headers.txt` اور `portal.gateway.binding.json` بھی تیار کرتا ہے ، جو نمونے ہیں جو پورٹل مواد سے منسلک ہونے کے لئے DG-3 کی ضروریات کو پورا کرتے ہیں:

- `portal.gateway.headers.txt` میں HTTP ہیڈرز کا ایک مکمل بلاک ہے (بشمول `Sora-Name` ، Docusaurus ، `Sora-Proof` ، CSP ، HSTS ، اور `Sora-Route-Binding`) جس میں ایج گیٹ ویز کو ہر ردعمل میں پیسٹ کرنا چاہئے۔
- `portal.gateway.binding.json` ایک ہی معلومات کو مشین پڑھنے کے قابل شکل میں ریکارڈ کرتا ہے تاکہ ٹکٹ اور آٹومیشن کو تبدیل کیا جاسکے بغیر شیل آؤٹ پٹ کو کھرچنے کے میزبان/CID کا موازنہ کیا جاسکے۔

یہ خود بخود `cargo xtask soradns-binding-template` کے ذریعہ تیار کیا جاتا ہے اور عرف ، مینی فیسٹ فنگر پرنٹ ، اور گیٹ وے کے میزبان نام کو `sorafs-pin-release.sh` پر منتقل کرتا ہے۔ تخلیق نو یا تخصیص کے لئے:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

`--csp-template` ، `--permissions-template` ، یا `--hsts-template` کو ضرورت پڑنے پر پہلے سے طے شدہ ہیڈر ٹیمپلیٹس کو اوور رائڈ کرنے کے لئے پاس کریں ، اور ان کو `--no-*` کیز کے ساتھ جوڑ کر پورے ہیڈر کو دور کریں۔

ہیڈر سیکشن کو سی ڈی این تبدیلی کی درخواست سے منسلک کریں اور JSON دستاویز کو گیٹ وے مشین لائن پر دھکیلیں تاکہ میزبان اپ گریڈ ریلیز گائیڈز سے مماثل ہو۔

توثیق اسسٹنٹ اسکرپٹ خود بخود چلتا ہے تاکہ DG-3 ٹکٹوں میں تازہ ترین ثبوت موجود ہوں۔ JSON بائنڈنگ میں ترمیم کرتے وقت اسے دستی طور پر دوبارہ شروع کریں:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

یہ کمانڈز `Sora-Proof` پے لوڈ کو کھولیں ، اس بات کو یقینی بنائیں کہ `Sora-Route-Binding` ڈیٹا مینی فیسٹ سی آئی ڈی اور میزبان نام سے مماثل ہے ، اور اگر کسی ہیڈر کو غلط استعمال کیا جاتا ہے تو ناکام ہوجاتے ہیں۔ جب CI کے باہر کمانڈ چلاتے ہو تو کنسول آؤٹ پٹ کو باقی نوادرات کے ساتھ رکھیں۔

> ** ڈی این ایس ڈسکرپٹر انضمام: ** `portal.dns-cutover.json` اب ایک `gateway_binding` سیکشن کو مربوط کرتا ہے جو ان فائلوں (راستے ، مواد سی آئی ڈی ، پروف اسٹیٹس ، اور ہیڈر ٹیمپلیٹ) کی طرف اشارہ کرتا ہے ** کے ساتھ ساتھ ** ایک `route_plan` سیکشن جو Sigstore کی طرف اشارہ کرتا ہے۔ ان حصوں کو ہر DG-3 ٹکٹ میں شامل کریں تاکہ جائزہ لینے والے `Sora-Name/Sora-Proof/CSP` اقدار کا موازنہ کرسکیں اور اس بات کو یقینی بنائیں کہ اپ گریڈ/رول بیک منصوبے محفوظ شدہ دستاویزات کو کھولے بغیر ثبوت کے پیکیج سے مماثل ہیں۔

## مرحلہ 9 - تعیناتی مانیٹر چلائیں** دستاویزات -3 سی ** روڈ میپ کے لئے جاری ثبوت کی ضرورت ہے کہ پورٹل ، آئی ٹی ایجنٹ آزمائیں ، اور پورٹل لنکس تعیناتی کے بعد برقرار رہیں۔ 7-8 اقدامات کے فورا. بعد یونیفائیڈ مانیٹر چلائیں اور اسے اپنی شیڈول تحقیقات سے جوڑیں:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` کنفیگریشن فائل کو لوڈ کرتا ہے (آریگرام کے لئے `docs/portal/docs/devportal/publishing-monitoring.md` دیکھیں) اور تین چیکوں کو انجام دیتا ہے: CSP/اجازت نامے سے متعلق پولیس کے ساتھ گیٹ وے کے راستے ، ENTSTRECS کی پروسیس (اختیاری طور پر `/metrics`) اور ایک گیٹ وے بائنڈنگ چیک (`/metrics`) کی کوشش کریں۔ ایک چیک عرف/مینی فیسٹ کے ساتھ سورہ-مواد-سی آئی ڈی ویلیو۔
- جب کوئی تحقیقات ناکام ہوجاتی ہے تو غیر صفر سے باہر نکلنے کے ساتھ کمانڈ ختم کردیتا ہے تاکہ CI/CRON/آپریشن ٹیمیں اپ گریڈ کو روک سکیں۔
- پاس `--json-out` اسٹیٹس کا ایک واحد JSON خلاصہ لکھتا ہے۔ `--evidence-dir` برآمدات `summary.json` ، `portal.json` ، `tryit.json` ، `binding.json` ، اور `checksums.sha256` تاکہ گورننس آڈیٹر دوبارہ شروع کیے بغیر نتائج کا موازنہ کرسکیں۔ `artifacts/sorafs/<tag>/monitoring/` کے تحت اس ڈائرکٹری کو Sigstore بنڈل اور DNS کٹ اوور ڈسکرپٹر کے ساتھ محفوظ کریں۔
- Grafana (`dashboards/grafana/docs_portal.json`) کی نگرانی اور برآمد آؤٹ پٹ اور الرٹ مینجر ڈرل ID کو ریلیز ٹکٹ میں منسلک کریں تاکہ بعد میں DOCS-3C SLO قابل آیت ہو۔ کنٹرول دستی `docs/portal/docs/devportal/publishing-monitoring.md` پر درج ہے۔

گیٹ وے کی تحقیقات کو HTTPS کی ضرورت ہوتی ہے اور `http://` پتے کو مسترد کرتے ہیں جب تک کہ `allowInsecureHttp` سیٹ اپ میں فعال نہ ہو۔ پیداوار/ٹیسٹ کے ماحول میں TLS کو برقرار رکھیں اور صرف مقامی پیش نظاروں کے لئے استثناء کو قابل بنائیں۔

ایک بار گیٹ وے دستیاب ہونے کے بعد آپ بلڈکائٹ/کرون میں `npm run monitor:publishing` کے ذریعے مانیٹر کو خود کار بنا سکتے ہیں۔ پروڈکشن لنکس کے ساتھ بھی ریلیز کے مابین صحت کی مسلسل جانچ پڑتال ہوتی ہے۔

## `sorafs-pin-release.sh` کے ذریعے آٹومیشن

`docs/portal/scripts/sorafs-pin-release.sh` اقدامات 2-6 مرتب کرتا ہے۔ وہ ہے:

1. `build/` ایک عصبی ٹربال میں آرکائو کیا گیا ہے ،
2. `car pack` ، `manifest build` ، `manifest sign` ، `manifest verify-signature` ، `proof verify` پر عملدرآمد کرتا ہے ،
3. اختیاری طور پر `manifest submit` پر عمل درآمد ہوتا ہے (بشمول عرف بائنڈنگ) جب Torii اسناد دستیاب ہوں گے ،
4. `artifacts/sorafs/portal.pin.report.json` لکھتے ہیں اور اختیاری `portal.pin.proposal.json` ، DNS کٹ اوور ڈسکرپٹر ، اور گیٹ وے بائنڈنگ پیکیج (`portal.gateway.binding.json` بلاک ہیڈرز کے ساتھ) لہذا گورننس ، نیٹ ورکنگ ، اور آپریشن ٹیمیں CI لاگز کو پڑھے بغیر ڈائریکٹریوں کا جائزہ لے سکتی ہیں۔

اسکرپٹ چلانے سے پہلے `PIN_ALIAS` ، `PIN_ALIAS_NAMESPACE` ، `PIN_ALIAS_NAME` ، اور (اختیاری طور پر) `PIN_ALIAS_PROOF_PATH` سیٹ کریں۔ خشک تجربات کے لئے `--skip-submit` استعمال کریں۔ گٹ ہب کا ورک فلو `perform_submit` کے ذریعے اس کو تبدیل کرتا ہے۔

## مرحلہ 8 - OpenAPI وضاحتیں اور SBOM پیکیجز تعینات کریں

DOCS-7 کا تقاضا ہے کہ گیٹ وے بلڈ ، OpenAPI اور SBOM کی وضاحتیں اسی طرح کے راستے سے گزریں۔ تین موجودہ ٹولز کا احاطہ:

1. ** دوبارہ تخلیق اور اشارے کی وضاحتیں۔ **

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   تاریخی اسنیپ شاٹ (جیسا کہ `2025-q3`) کو بچانے کے لئے `--version=<label>` استعمال کریں۔ مددگار اسنیپ شاٹ کو `static/openapi/versions/<label>/torii.json` پر لکھتا ہے ، اسے `versions/current` پر آئینہ دیتا ہے ، اور SHA-256 ڈیٹا ، مینی فیسٹ اسٹیٹس اور اپ ڈیٹ کی تاریخ کے ساتھ `static/openapi/versions.json` کو اپ ڈیٹ کرتا ہے۔ ڈویلپرز کی ویب سائٹ کاپی کے انتخاب اور فنگر پرنٹ/دستخطی معلومات کو ظاہر کرنے کے لئے اس اشارے کو پڑھتی ہے۔ اگر `--version` حذف ہو گیا ہے تو ، پچھلے عہدے باقی رہیں گے اور صرف `current` اور `latest` کو اپ ڈیٹ کیا جائے گا۔

   مینی فیسٹ نے SHA-256/BLAKE3 فنگر پرنٹ کو اپنی گرفت میں لے لیا تاکہ گیٹ وے `Sora-Proof` کو `/reference/torii-swagger` پر ڈال سکتا ہے۔

2. ** سائکلونڈ ایکس ایس بی او ایم ایس ریلیز۔ آؤٹ پٹ کو تعمیراتی نمونے کے قریب رکھیں:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. ** ہر پے لوڈ کو کار کے اندر پیکج کریں۔ **

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
   ```مرکزی سائٹ کے لئے `manifest build` / Sigstore جیسے اقدامات پر عمل کریں ، اور ہر نوادرات کے لئے عرفات مرتب کریں (جیسے کہ تصریح کے لئے `docs-openapi.sora` اور دستخط شدہ SBOM پیکیج کے لئے `docs-sbom.sora`)۔ اس سے ہر پے بوجھ سے سورڈن ، گار اور رول بیک ٹکٹ بندھے ہوئے ہیں۔

4. ** بھیجنا اور پابند کرنا۔

گیٹ وے بلڈ کے ساتھ OpenAPI/SBOM رجسٹر کو محفوظ کرنے سے یہ یقینی بنانے میں مدد ملتی ہے کہ ہر ٹکٹ میں پیکر کو دوبارہ شروع کیے بغیر ڈائریکٹریوں کا مکمل سیٹ موجود ہے۔

### آٹومیشن اسسٹنٹ (CI/پیکیج اسکرپٹ)

`./ci/package_docs_portal_sorafs.sh` 1-8 اقدامات کو ایک کمانڈ میں جوڑتا ہے۔ اسسٹنٹ کرتا ہے:

- گیٹ وے سیٹ اپ (`npm ci` ، SYNC OpenAPI/Norito ، ٹیسٹ ویجٹ)۔
- گیٹ وے ، OpenAPI اور SBOM کے لئے کاریں اور منشور جاری کرنا `sorafs_cli` کے ذریعے۔
- اختیاری طور پر `sorafs_cli proof verify` (`--proof`) کو نافذ کریں اور Sigstore (`--sign` ، `--sigstore-provider` ، `--sigstore-audience`) کے ذریعے دستخط کریں۔
- تمام اثاثوں کو `artifacts/devportal/sorafs/<timestamp>/` کے تحت رکھیں اور CI/ریلیز ٹولز پر `package_summary.json` لکھیں۔
- تازہ ترین آپریشن کی نشاندہی کرنے کے لئے `artifacts/devportal/sorafs/latest` کو اپ ڈیٹ کیا گیا۔

مثال (Sigstore+POR کے ساتھ مکمل لائن):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

سب سے اہم میڈیا:

- `--out <dir>` - جڑ کے اثاثوں کو تبدیل کریں (پہلے سے طے شدہ ٹائم اسٹیمپ فولڈرز)۔
- `--skip-build` - موجودہ `docs/portal/build` کو دوبارہ استعمال کریں۔
- `--skip-sync-openapi` - بائی پاس `npm run sync-openapi` جب `cargo xtask openapi` کریٹس تک رسائی حاصل نہیں کرسکتا۔
- `--skip-sbom` - جب دستیاب نہ ہونے پر (انتباہ کے ساتھ) `syft` پر کال نہ کریں۔
- `--proof` - ہر کار/منشور جوڑی کے لئے `sorafs_cli proof verify` کو نافذ کریں۔
- `--sign` - `sorafs_cli manifest sign` کو نافذ کریں۔

آؤٹ پٹ کے لئے `docs/portal/scripts/sorafs-pin-release.sh` استعمال کریں۔ پورٹل/OpenAPI/SBOM منشور کو مرتب کرتا ہے اور اس پر دستخط کرتا ہے اور `portal.additional_assets.json` پر اضافی میٹا ڈیٹا ریکارڈ کرتا ہے۔ کیز `--openapi-*` ، `--portal-sbom-*` اور `--openapi-sbom-*` کو ہر نمونے کے لئے عرف مرتب کرنے کے لئے `--openapi-sbom-source` کے ذریعے SBOM ماخذ کو بائی پاس کرنے کے لئے ، کچھ پے لوڈ (`--skip-openapi`/`--skip-sbom`) کو چھوڑیں `--syft-bin`۔

اسکرپٹ تمام احکامات دکھاتا ہے۔ لاگ کو `package_summary.json` کے ساتھ ریلیز ٹکٹ پر کاپی کریں تاکہ جائزہ لینے والے دستی شیل آؤٹ پٹ کو ٹریک کیے بغیر ہضم اور میٹا ڈیٹا کا موازنہ کرسکیں۔

## مرحلہ 9 - گیٹ وے + سورادن کی تصدیق کریں

کٹ اوور کا اعلان کرنے سے پہلے ، یہ ثابت کریں کہ نیا عرف سوردنس کے توسط سے حل ہوا ہے اور گیٹ ویز نئے ثبوت نصب کرتے ہیں:

1. ** تحقیقات کے گیٹ کو آن کریں۔ اصل آپریشن کے لئے ، ٹیسٹ کو مطلوبہ میزبان نام کی طرف اشارہ کریں:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   تحقیقات `Sora-Name` ، `Sora-Proof` ، اور `Sora-Proof-Status` ہیڈر `docs/source/sorafs_alias_policy.md` کے مطابق ڈیکوڈ کرتی ہے اور ناکام ہوجاتی ہے جب مینیفیسٹ فنگر پرنٹ ، ٹی ٹی ایل ، یا گار بائنڈنگ ختم ہوجاتی ہے۔

   ہلکا پھلکا اسپاٹ چیک کے لئے (مثال کے طور پر ، جب صرف پابند بنڈل
   تبدیل شدہ) ، `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>` چلائیں۔
   مددگار پکڑے گئے بائنڈنگ بنڈل کی توثیق کرتا ہے اور رہائی کے لئے آسان ہے
   وہ ٹکٹ جن کو صرف مکمل تحقیقات کی مشق کی بجائے پابند تصدیق کی ضرورت ہوتی ہے۔

2. ** مشقوں کی ڈائریکٹریز جمع کریں۔ Soradns کے راستے کو چیک کرنے کے لئے `--host docs.sora` سیٹ کریں۔3. ** DNS پابندیوں کو چیک کریں۔ ریزولور آپریٹرز `tools/soradns-resolver` کے ذریعے دوبارہ چل سکتے ہیں۔ JSON منسلک کرنے سے پہلے ، میزبان میپنگ ، میٹا ڈیٹا ، اور ٹیلی میٹری لیبل چیک کرنے کے لئے `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]` چلائیں۔ اسسٹنٹ دستخط شدہ گار کے ساتھ ساتھ `--json-out` کا خلاصہ جاری کرسکتا ہے۔
  جب کسی نئے GAR کا مسودہ تیار کرتے ہو تو ، `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...` کو ترجیح دیں ، اور `--manifest-cid` صرف اس وقت استعمال کریں جب مینی فیسٹ فائل غیر حاضر ہو۔ مددگار اب JSON سے براہ راست سی آئی ڈی اور بلیک 3 فنگر پرنٹ اخذ کرتا ہے ، خالی جگہوں کو ہٹاتا ہے ، ڈپلیکیٹ لیبلوں کو ہٹاتا ہے ، ان کو اکٹھا کرتا ہے ، اور تحریری طور پر تعی .ن کو یقینی بنانے کے ل writing لکھنے سے پہلے سی ایس پی/ایچ ایس ٹی/اجازت-پالیسی ٹیمپلیٹس کو شامل کرتا ہے۔

4. ** عرفی میٹرکس کی نگرانی کریں۔ ** `torii_sorafs_alias_cache_refresh_duration_ms` اور `torii_sorafs_gateway_refusals_total{profile="docs"}` کو اسکرین پر رکھیں۔ دونوں کو `dashboards/grafana/docs_portal.json` پر دکھایا گیا ہے۔

## مرحلہ 10 - مانیٹر اور پیکیج کے ثبوت

- ** پلیٹیں۔
- ** محفوظ شدہ دستاویزات۔
- ** ریلیز پیکیج۔
- ** مشقیں لاگ ان۔
- ** ٹکٹ کے لنکس۔

## مرحلہ 11 - ملٹی سورس بازیافت ورزش اور اسکور بورڈ گائیڈز

SoraFS تعیناتی کے لئے اب DNS/گیٹ وے ڈائریکٹریوں کے ساتھ ملٹی سورس بازیافت ڈائریکٹریز (DOCS-7/SF-6) کی ضرورت ہے۔ مینی فیسٹ انسٹال کرنے کے بعد:

1. ** براہ راست منشور پر `sorafs_fetch` چلائیں۔

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

   - پہلے مینی فیسٹ (جیسے `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) میں اشارہ کردہ فراہم کنندہ اشتہارات حاصل کریں اور ان کو `--provider-advert name=path` کے ذریعے منتقل کریں تاکہ یہ یقینی بنایا جاسکے کہ صلاحیتوں کا لامحالہ اندازہ کیا جائے۔ CI میں فکسچر کو دوبارہ شروع کرتے وقت صرف ** `--allow-implicit-provider-metadata` استعمال کریں۔
   - جب اضافی خطے موجود ہیں تو ، ہر کیشے/عرف کے ثبوت حاصل کرنے کے لئے اپنے فراہم کنندگان کے ساتھ دوبارہ بوٹ کریں۔

2. ** آؤٹ پٹ کو محفوظ کریں۔

3. ** پیمائش کو اپ ڈیٹ کریں۔

4. ** الرٹ کے قواعد چیک کریں۔ ** `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں اور پرومٹول آؤٹ پٹ منسلک کریں۔

5. ** CI میں ضم ہوجائیں۔

## اپ گریڈ ، نوٹ اور رول بیک

1. ** اپ گریڈ: ** اسٹیجنگ اور پروڈکشن کے لئے الگ الگ عرفی نام رکھیں۔ `manifest submit` کو اسی مینی فیسٹ میں واپس کرکے اور `--alias-namespace/--alias-name` کو پروڈکشن عرف میں تبدیل کرکے اپ گریڈ کریں۔
2. ** نگرانی: ** `docs/portal/docs/devportal/observability.md` پر `docs/source/grafana_sorafs_pin_registry.json` اور خصوصی گیٹ وے کی تحقیقات استعمال کریں۔
3. ** کالعدم: ** رول بیک کو دوبارہ کرنے کے لئے ، `sorafs_cli manifest submit --alias ... --retire` کے ذریعے پچھلے مینی فیسٹ (یا موجودہ عرف کو کھینچیں) کو دوبارہ جمع کریں۔

## CI ٹیمپلیٹ

کم سے کم پائپ لائن میں شامل ہونا چاہئے:1. بلڈ + لنٹ (`npm ci` ، `npm run build` ، چیکسس تیار کریں)۔
2. پیکیجنگ (`car pack`) اور ظاہر حساب کتاب۔
3. نوکری ٹوکن OIDC (`manifest sign`) کا استعمال کرتے ہوئے دستخط کریں۔
4. اثاثوں کو بڑھانا (کار ، منشور ، بنڈل ، منصوبہ ، خلاصہ)۔
5. پن رجسٹری کو بھیجنا:
   - درخواستیں کھینچیں -> `docs-preview.sora`۔
   - ٹیگز/محفوظ شاخیں -> اپ گریڈ آؤٹ پٹ عرف۔
6. تکمیل سے پہلے ثبوت کی تصدیق کرنے کے لئے تحقیقات + گیٹس چلائیں۔

`.github/workflows/docs-portal-sorafs-pin.yml` ان اقدامات کو دستی ورژن سے جوڑتا ہے۔ ورک فلو کرتا ہے:

- پورٹل کی تعمیر/جانچ ،
`scripts/sorafs-pin-release.sh` کے ذریعے انکپسولیشن کو بلڈ کریں ،
- گٹ ہب OIDC کے ذریعے مینیفیسٹ بنڈل پر دستخط/تصدیق کرنا ،
- اپ لوڈ کار/منشور/بنڈل/پلان/پروف خلاصے بطور نمونے ،
- (اختیاری طور پر) جب راز دستیاب ہوں تو منشور اور لنک عرف بھیجیں۔

چلانے سے پہلے درج ذیل راز/متغیرات مرتب کریں:

| نام | مقصد |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | میزبان Torii جو `/v2/sorafs/pin/register` کو بے نقاب کرتا ہے۔ |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | ایپوچ ID ٹرانسمیٹر کے ساتھ رجسٹرڈ ہے۔ |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | منشور بھیجنے کے لئے اتھارٹی پر دستخط کرنا۔ |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | `perform_submit` = `true` پر عرف ٹوپل پابند ہے۔ |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | بیس 64 انکوڈڈ عرف (اختیاری) کے لئے بنڈل پروف۔ |
| `DOCS_ANALYTICS_*` | موجودہ تجزیات/تحقیقات کا اسکور۔ |

ایکشن انٹرفیس کے ذریعے ورک فلو چلائیں:

1. `alias_label` (بطور `docs.sora.link`) ، اختیاری `proposal_alias` ، اور اختیاری `release_tag` فراہم کریں۔
2. `perform_submit` کو غیر فعال چھوڑیں Torii (ڈرائی رن) کو چھوئے بغیر اثاثوں کو پیدا کرنے کے لئے یا عرف کو براہ راست شائع کرنے کے لئے اسے چالو کریں۔

`docs/source/sorafs_ci_templates.md` عام ٹیمپلیٹس کے لئے ایک حوالہ بنی ہوئی ہے ، لیکن پورٹل کا ورک فلو روزانہ کی رہائی کے لئے ترجیحی راستہ ہے۔

## چیک لسٹ

- [] `npm run build` ، `npm run test:*` اور `npm run check:links` کامیابی۔
- [] `build/checksums.sha256` اور `build/release.json` کو بطور اثاثہ بچائیں۔
- [] `artifacts/` کے تحت کار ، منصوبہ ، منشور اور خلاصہ بنائیں۔
- [] Sigstore بنڈل اور ریکارڈ کے ساتھ علیحدہ دستخط بچائیں۔
- [] بھیجتے وقت `portal.manifest.submit.summary.json` اور `portal.manifest.submit.response.json` محفوظ کریں۔
- [] آرکائیو `portal.pin.report.json` (اور دستیاب ہونے پر `portal.pin.proposal.json`)۔
- [] `proof verify` اور `manifest verify-signature` کے ریکارڈ محفوظ کریں۔
- [] تازہ ترین Grafana بورڈز اور کامیاب TRY-IT تحقیقات۔
- [] ریلیز ٹکٹ کے ساتھ مراجعت نوٹ (پچھلا مینی فیسٹ ID + ڈائجسٹ عرف) منسلک کریں۔