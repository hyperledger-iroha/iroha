---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## جائزہ

یہ پلے بوک ** دستاویزات -7 ** (اشاعت SoraFS) اور ** دستاویزات -8 ** (CI/CD پن آٹومیشن) روڈ میپ آئٹمز کو ڈویلپر پورٹل کے عملی عمل میں ترجمہ کرتی ہے۔ اس میں بلڈ/لنٹ مرحلے ، پیکیجنگ SoraFS کا احاطہ کیا گیا ہے ، Sigstore کے ذریعے ظاہر ہوتا ہے ، عرفیوس کو فروغ دیتا ہے ، چیکنگ اور رول بیک ٹریننگ تاکہ ہر پیش نظارہ اور رہائی قابل تولیدی اور قابل اظہار ہو۔

بہاؤ یہ فرض کرتا ہے کہ آپ کے پاس `sorafs_cli` بائنری ہے (`--features cli` کے ساتھ بنایا گیا ہے) ، Torii اختتامی نقطہ تک PIN-REGGISTY کے حقوق کے ساتھ رسائی ، اور OIDC ساکھ Sigstore کے لئے۔ سی آئی والٹ میں طویل عرصے تک راز (`IROHA_PRIVATE_KEY` ، `SIGSTORE_ID_TOKEN` ، Torii ٹوکن) اسٹور کریں۔ مقامی لانچ ان کو شیل میں برآمد سے اٹھا سکتے ہیں۔

## شرائط

- `npm` یا `pnpm` کے ساتھ نوڈ 18.18+۔
- `sorafs_cli` `cargo run -p sorafs_car --features cli --bin sorafs_cli` سے۔
- URL Torii ، جو `/v2/sorafs/*` کھولتا ہے ، نیز ایک اکاؤنٹ/نجی کلید جو ظاہر اور عرفی نام بھیجنے کے قابل ہے۔
- OIDC جاری کرنے والا (گٹ ہب ایکشنز ، گٹ لیب ، ورک بوجھ کی شناخت ، وغیرہ) رہائی کے لئے `SIGSTORE_ID_TOKEN`۔
- اختیاری: خشک رن کے لئے `examples/sorafs_cli_quickstart.sh` اور Github/Gitlab ورک فلوز ٹیمپلیٹس کے لئے `docs/source/sorafs_ci_templates.md`۔
- OAuth متغیرات کو ترتیب دیں (`DOCS_OAUTH_*`) اور عملدرآمد کریں
  [سیکیورٹی سخت کرنے والی چیک لسٹ] (./security-hardening.md) لیب کے باہر تعمیر کو فروغ دینے سے پہلے۔ اب پورٹل بلڈ کریش اگر متغیرات غائب ہیں یا ٹی ٹی ایل/کھینچنے والے پیرامیٹرز درست ونڈوز سے باہر ہیں۔ صرف ایک وقت کے مقامی پیش نظاروں کے لئے `DOCS_OAUTH_ALLOW_INSECURE=1` استعمال کریں۔ رہائی کے کام سے قلم ٹیسٹ کے ثبوت منسلک کریں۔

## مرحلہ 0 - بنڈل پراکسی کو ٹھیک کریں اسے آزمائیں

پیش نظارہ کو نیٹ لائف یا گیٹ وے کے فروغ دینے سے پہلے ، اس پرکسی پراکسی اور دستخط شدہ OpenAPI کے ذرائع کو ہضم کرنے کے ذرائع کو ایک تعی .ن بنڈل میں ظاہر کریں:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` پراکسی/تحقیقات/رول بیک مددگاروں کی کاپی کرتا ہے ، OpenAPI کے دستخط کی جانچ کرتا ہے اور `release.json` پلس `checksums.sha256` لکھتا ہے۔ اس بنڈل کو نیٹ لیف/SoraFS گیٹ وے ٹکٹ سے منسلک کریں تاکہ جائزہ لینے والے بغیر کسی تعمیر نو کے عین مطابق پراکسی ذرائع اور Torii ہدف کے اشارے کو دوبارہ پیش کرسکیں۔ بنڈل یہ بھی ریکارڈ کرتا ہے کہ آیا کلائنٹ بیئرر ٹوکن فعال ہیں (`allow_client_auth`) تاکہ رول آؤٹ پلان اور سی ایس پی کے قواعد ہم آہنگی میں رہیں۔

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

`npm run build` خود بخود `scripts/write-checksums.mjs` شروع کرتا ہے ، تخلیق کرتے ہیں:

- `build/checksums.sha256` - SHA256 `sha256sum -c` کے لئے مینی فیسٹ۔
- `build/release.json` - میٹا ڈیٹا (`tag` ، `generated_at` ، `source`) ہر کار/مینی فیسٹ کے لئے۔

دونوں فائلوں کو کار سمری کے ساتھ مل کر محفوظ کریں تاکہ جائزہ لینے والے بغیر کسی تعمیر نو کے پیش نظارہ نمونے کا موازنہ کرسکیں۔

## مرحلہ 2 - پیکیج جامد اثاثے

آؤٹ پٹ ڈائرکٹری Docusaurus پر کار پیکر چلائیں۔ ذیل کی مثال `artifacts/devportal/` پر تمام نمونے لکھتی ہے۔

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

خلاصہ JSON پروف شیڈولنگ کے لئے حصوں ، ہضموں اور اشارے کی تعداد حاصل کرتا ہے ، جو اس کے بعد `manifest build` اور CI ڈیش بورڈز کے ذریعہ استعمال ہوتا ہے۔## مرحلہ 2B - پیک OpenAPI اور SBOM ساتھی

DOCS-7 کے لئے پورٹل سائٹ ، اسنیپ شاٹ OpenAPI اور SBOM پے لوڈ کو الگ الگ منشور کے طور پر شائع کرنے کی ضرورت ہے تاکہ گیٹ وے ہر نمونے کے لئے `Sora-Proof`/`Sora-Content-CID` ہیڈر شامل کرسکیں۔ ہیلپر (`scripts/sorafs-pin-release.sh`) پہلے ہی OpenAPI ڈائرکٹری (`static/openapi/`) اور `syft` سے SBOM کو الگ الگ کاروں `openapi.*`/Sigstore میں پیک کرتا ہے اور میٹا ڈیٹا کو لکھتا ہے۔ دستی بہاؤ میں ، ہر پے لوڈ کے لئے اپنے اپنے سابقہ ​​اور میٹا ڈیٹا (مثال کے طور پر `--car-out "$OUT"/openapi.car` پلس `--metadata alias_label=docs.sora.link/openapi`) کے ساتھ 2-4 اقدامات دہرائیں۔ ڈی این ایس کو تبدیل کرنے سے پہلے ہر مینی فیسٹ/عرف جوڑی کو Torii (OpenAPI ، پورٹل SBOM ، OpenAPI SBOM) میں رجسٹر کریں تاکہ گیٹ وے تمام شائع شدہ نمونے کے لئے تیز ثبوت جاری کرسکے۔

## مرحلہ 3 - مینی فیسٹ اکٹھا کریں

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

ریلیز ونڈو کے لئے پن پالیسی کے جھنڈے مرتب کریں (مثال کے طور پر ، کینریوں کے لئے `--pin-storage-class hot`)۔ JSON آپشن اختیاری ہے ، لیکن جائزہ لینے کے لئے آسان ہے۔

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

بنڈل JWT کو ذخیرہ کیے بغیر ٹوکن کے منشور ڈائجسٹ ، حصہ ڈائجسٹس ، اور بلیک 3 ہیش OIDC پر قبضہ کرتا ہے۔ اسٹور بنڈل اور علیحدہ دستخط ؛ پروڈکشن پروموشنز دوبارہ دستخط کرنے کے بجائے ایک ہی نمونے کو دوبارہ استعمال کرسکتے ہیں۔ مقامی رنز فراہم کنندہ کے جھنڈوں کو `--identity-token-env` (یا ماحول میں `SIGSTORE_ID_TOKEN` سیٹ کریں) کے ساتھ تبدیل کرسکتے ہیں جب بیرونی OIDC مددگار ایک ٹوکن جاری کرتا ہے۔

## مرحلہ 5 - پن رجسٹری کو بھیجیں

Torii پر دستخط شدہ منشور (اور حصہ منصوبہ) بھیجیں۔ ہمیشہ ایک سمری کی درخواست کریں تاکہ رجسٹری/عرفی اندراج قابل آیت ہو۔

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

رول آؤٹ پیش نظارہ یا کینری عرف (`docs-preview.sora`) کے لئے ، ایک منفرد عرف کے ساتھ جمع کرانے کو دہرائیں تاکہ QA پروڈکشن پروموشن سے پہلے مواد کی جانچ کرسکے۔

عرف بائنڈنگ کے لئے تین فیلڈز کی ضرورت ہے: `--alias-namespace` ، `--alias-name` ، اور `--alias-proof`۔ عرف درخواست کی منظوری کے بعد گورننس ایک پروف بنڈل (BASE64 یا Norito بائٹس) جاری کرتا ہے۔ اسے CI رازوں میں اسٹور کریں اور `manifest submit` سے پہلے اسے فائل کے طور پر پیش کریں۔ اگر آپ صرف DNS کو تبدیل کیے بغیر ہی ظاہر کرنا چاہتے ہیں تو عرف پرچم خالی چھوڑ دیں۔

## مرحلہ 5B - گورننس کی تجویز پیدا کریں

ہر منشور کے ساتھ پارلیمنٹ کی تجویز پیش کی جانی چاہئے تاکہ کوئی بھی سورہ شہری مراعات یافتہ چابیاں تک رسائی کے بغیر تبدیلی کرسکے۔ جمع کرانے/سائن کے بعد کریں:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` نے کیننیکل انسٹرکشن `RegisterPinManifest` ، CUNK DISENCE ، پالیسی اور عرف اشارے کو ٹھیک کیا ہے۔ اسے گورننس ٹکٹ یا پارلیمنٹ کے پورٹل سے منسلک کریں تاکہ مندوبین بغیر کسی تعمیر نو کے پے لوڈ کا موازنہ کرسکیں۔ ٹیم Torii کیز استعمال نہیں کرتی ہے ، لہذا کوئی بھی شہری مقامی طور پر تجویز تیار کرسکتا ہے۔

## مرحلہ 6 - ثبوت اور ٹیلی میٹری چیک کریں

پن کے بعد ، تعصب کی جانچ پڑتال کریں:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```- `torii_sorafs_gateway_refusals_total` اور `torii_sorafs_replication_sla_total{outcome="missed"}` پر نگاہ رکھیں۔
- `npm run probe:portal` کو TRY-IT پراکسی اور ریکارڈ شدہ لنکس کو تازہ مواد پر چیک کرنے کے لئے چلائیں۔
- سے نگرانی کے ثبوت جمع کریں
  [پبلشنگ اینڈ مانیٹرنگ] (./publishing-monitoring.md) DOCS-3C مشاہداتی گیٹ کی تعمیل کرنا۔ ہیلپر اب ایک سے زیادہ `bindings` (سائٹ ، OpenAPI ، پورٹل SBOM ، OpenAPI SBOM) کو قبول کرتا ہے اور `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` کو `hostname` کے ذریعے `Sora-Proof`/`Sora-Content-CID` کی ضرورت ہے۔ نیچے دیئے گئے کال میں ایک واحد JSON خلاصہ اور ثبوت بنڈل لکھتا ہے (`portal.json` ، `tryit.json` ، `binding.json` ، `checksums.sha256`) ریلیز ڈائرکٹری:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## مرحلہ 6A - منصوبہ بندی گیٹ وے سرٹیفکیٹ

گار پیکٹ بنانے سے پہلے TLS SAN/چیلنج پلان بنائیں تاکہ گیٹ وے ٹیم اور DNS منظوری اسی ثبوت کے ساتھ کام کریں۔ نیا مددگار ڈی جی 3 ان پٹ ، لسٹنگ کیننیکل وائلڈ کارڈ میزبان ، خوبصورت میزبان سینز ، DNS-01 لیبل اور سفارش کردہ ACME چیلنجز:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

ریلیز کے بنڈل کے ساتھ JSON کا ارتکاب کریں (یا کسی تبدیلی کے ٹکٹ سے منسلک ہوں) تاکہ آپریٹرز `torii.sorafs_gateway.acme` کنفیگریشن میں SAN اقدار داخل کرسکیں ، اور GAR جائزہ لینے والے دوبارہ چلائے بغیر کیننیکل/خوبصورت نقشہ سازی کی جانچ کرسکتے ہیں۔ ایک ریلیز میں ہر لاحقہ کے لئے اضافی `--name` شامل کریں۔

## مرحلہ 6 بی - کیننیکل میزبان میپنگز حاصل کریں

GAR پے لوڈ کو ٹیمپلیٹ کرنے سے پہلے ، ہر عرف کے لئے ایک عزم میزبان نقشہ سازی کا عہد کریں۔ `cargo xtask soradns-hosts` ہر ایک `--name` کو ایک کیننیکل لیبل (`<base32>.gw.sora.id`) میں ہیش کرتا ہے ، مطلوبہ وائلڈ کارڈ (`*.gw.sora.id`) جاری کرتا ہے اور ایک خوبصورت میزبان (`<alias>.gw.sora.name`) کو آؤٹ پٹ کرتا ہے۔ ریلیز نمونے میں آؤٹ پٹ کو محفوظ کریں تاکہ DG-3 جائزہ لینے والے GAR جمع کرانے کے ساتھ ہی نقشہ سازی کی جانچ کرسکیں:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

جب GAR یا گیٹ وے بائنڈنگ JSON میں مطلوبہ میزبانوں پر مشتمل نہیں ہوتا ہے تو جلدی سے ناکام ہونے کے لئے `--verify-host-patterns <file>` استعمال کریں۔ ہیلپر متعدد فائلوں کو قبول کرتا ہے ، لہذا ایک کمانڈ میں گار ٹیمپلیٹ اور `portal.gateway.binding.json` دونوں کی جانچ کرنا آسان ہے:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

ایک سمری JSON اور توثیق لاگ کو DNS/گیٹ وے چینج ٹکٹ پر منسلک کریں تاکہ آڈیٹر کیننیکل/وائلڈ کارڈ/خوبصورت میزبانوں کی تصدیق کے بغیر دوبارہ لانچ کیے بغیر تصدیق کرسکیں۔ نئے عرف کو شامل کرتے وقت کمانڈ کو دوبارہ چلائیں تاکہ GAR کی تازہ کارییں ایک ہی ثبوت کے وارث ہوں۔

## مرحلہ 7 - ڈی این ایس کٹ اوور ڈسکرپٹر تیار کریں

پروڈکشن کٹ اوور کے لئے ایک آڈٹیبل چینج پیکٹ کی ضرورت ہوتی ہے۔ کامیاب جمع کرانے (عرف بائنڈنگ) کے بعد ، ہیلپر `artifacts/sorafs/portal.dns-cutover.json` تیار کرتا ہے ، اس کا ارتکاب کرتے ہیں:- میٹا ڈیٹا عرف بائنڈنگ (نام کی جگہ/نام/ثبوت ، منشور ڈائجسٹ ، Torii URL ، جمع کروایا گیا عہد ، اتھارٹی) ؛
- ریلیز سیاق و سباق (ٹیگ ، عرف لیبل ، منشور/کار کے راستے ، چنک پلان ، Sigstore بنڈل) ؛
- توثیق کے اشارے (تحقیقات کمانڈ ، عرف + Torii اختتامی نقطہ) ؛ اور
- اختیاری تبدیلی پر قابو پانے والے فیلڈز (ٹکٹ ID ، کٹ اوور ونڈو ، اوپس سے رابطہ ، پروڈکشن میزبان نام/زون) ؛
- `Sora-Route-Binding` ہیڈر (کیننیکل ہوسٹ/سی آئی ڈی ، ہیڈر + بائنڈنگ راہیں ، توثیق کے احکامات) سے میٹا ڈیٹا روٹ پروموشن تاکہ GAR پروموشن اور فال بیک مشقیں ایک ہی ثبوت کا حوالہ دیں۔
-تیار کردہ روٹ پلانٹ نمونے (`gateway.route_plan.json` ، ہیڈر ٹیمپلیٹس اور اختیاری رول بیک ہیڈر) ، تاکہ تبدیلی کے ٹکٹ اور CI لنٹ ہکس چیک کرسکیں کہ ہر DG-3 پیکٹ سے مراد کیننیکل پروموشن/رول بیک بیک منصوبوں سے ہے۔
- کیشے کے ناجائز ہونے کے لئے اختیاری میٹا ڈیٹا (صاف کریں اختتامی نقطہ ، آتھ متغیر ، JSON پے لوڈ اور مثال `curl`) ؛
- رول بیک پچھلے ڈسکرپٹر (ریلیز ٹیگ اور مینی فیسٹ ڈائجسٹ) کی طرف اشارہ کرتا ہے ، تاکہ ٹکٹوں کو تبدیل کیا جائے۔

اگر رہائی کے لئے کیشے پرج کی ضرورت ہوتی ہے تو ، ایک ڈسکرپٹر کے ساتھ ایک کیننیکل پلان تیار کریں:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

`portal.cache_plan.json` کو DG-3 پیکٹ سے منسلک کریں تاکہ آپریٹرز کو `PURGE` درخواستیں کرتے وقت ڈٹرمینسٹک میزبان/راستے (اور اسی طرح کے اشارے) ہوں۔ اختیاری کیشے ڈسکرپٹر سیکشن اس فائل کا براہ راست حوالہ دے سکتا ہے تاکہ تبدیلی پر قابو پانے والے جائزہ لینے والے یہ دیکھ سکیں کہ کٹ اوور کے دوران کون سے اختتامی نکات صاف ہوجاتے ہیں۔

ہر DG-3 پیکٹ میں بھی پروموشن + رول بیک چیک لسٹ کی ضرورت ہوتی ہے۔ اسے `cargo xtask soradns-route-plan` کے ذریعے تیار کریں تاکہ تبدیلی پر قابو پانے والے جائزہ لینے والے ہر عرف کے لئے عین مطابق پریفلائٹ/کٹ اوور/رول بیک بیک اقدامات کو ٹریک کرسکیں:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` نے کیننیکل/خوبصورت میزبانوں کو اپنی گرفت میں لے لیا ، صحت کی جانچ پڑتال کی یاد دہانی ، GAR بائنڈنگ اپڈیٹس ، کیشے صاف اور رول بیک ایکشنز۔ تبدیلی کا ٹکٹ جمع کروانے سے پہلے اسے گار/بائنڈنگ/کٹ اوور نمونے کے ساتھ بھی لگائیں تاکہ او پی ایس ایک ہی اقدامات کی مشق کرسکے۔

`scripts/generate-dns-cutover-plan.mjs` ایک ڈسکرپٹر تشکیل دیتا ہے اور `sorafs-pin-release.sh` سے لانچ کیا گیا ہے۔ دستی نسل کے لئے:

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

پن ہیلپر چلانے سے پہلے ماحولیاتی متغیر کے ذریعہ اختیاری میٹا ڈیٹا کو پُر کریں:

| متغیر | منزل |
| --- | --- |
| `DNS_CHANGE_TICKET` | ٹکٹ کی شناخت ، جو تفصیل کار میں محفوظ ہے۔ |
| `DNS_CUTOVER_WINDOW` | ISO8601 ونڈو کٹ اوور (مثال کے طور پر ، `2026-03-21T15:00Z/2026-03-21T15:30Z`)۔ |
| `DNS_HOSTNAME` ، `DNS_ZONE` | پروڈکشن میزبان نام + مستند زون۔ |
| `DNS_OPS_CONTACT` | آن کال عرف یا بڑھتی ہوئی رابطہ۔ |
| `DNS_CACHE_PURGE_ENDPOINT` | صاف کرنے والے اختتامی نقطہ ، وضاحتی کو لکھا ہوا۔ |
| `DNS_CACHE_PURGE_AUTH_ENV` | purge ٹوکن کے ساتھ env var (پہلے سے طے شدہ: `CACHE_PURGE_TOKEN`)۔ |
| `DNS_PREVIOUS_PLAN` | رول بیک میٹا ڈیٹا کے لئے پچھلے کٹ اوور ڈسکرپٹر کا راستہ۔ |

JSON کو DNS Change Change Change سے منسلک کریں تاکہ منظوری والے CI لاگز کو دیکھے بغیر مینی فیسٹ ڈائجسٹ ، عرف بائنڈنگ اور تحقیقات کے کمانڈ چیک کرسکیں۔ CLI FLAGS `--dns-change-ticket` ، `--dns-cutover-window` ، `--dns-hostname` ، `--dns-zone` ، `--ops-contact` ، `--cache-purge-endpoint` ، `--cache-purge-auth-env` اور Prometheus سے باہر ہے۔## مرحلہ 8 - حل کرنے والے زون فائل کنکال (اختیاری) پیدا کریں

جب پروڈکشن کٹ اوور ونڈو معلوم ہوجاتی ہے تو ، ریلیز اسکرپٹ خود بخود ایس این ایس زون فائل کنکال اور حل کرنے والا ٹکڑا پیدا کرسکتا ہے۔ env یا CLI کے توسط سے مطلوبہ DNS ریکارڈ اور میٹا ڈیٹا پاس کریں۔ ہیلپر کٹ اوور ڈسکرپٹر تیار کرنے کے فورا بعد `scripts/sns_zonefile_skeleton.py` پر کال کرے گا۔ براہ کرم کم از کم ایک A/AAAA/CNAME اور GAR ڈائجسٹ (BLAKE3-256 پر دستخط شدہ GAR پے لوڈ) فراہم کریں۔ اگر زون/میزبان نام جانا جاتا ہے اور `--dns-zonefile-out` غائب ہے تو ، مددگار `artifacts/sns/zonefiles/<zone>/<hostname>.json` پر لکھتا ہے اور `ops/soradns/static_zones.<hostname>.json` کو ایک حل کرنے والے ٹکڑے کے طور پر تخلیق کرتا ہے۔

| متغیر/پرچم | منزل |
| --- | --- |
| `DNS_ZONEFILE_OUT` ، `--dns-zonefile-out` | پیدا شدہ زون فائل کنکال کا راستہ۔ |
| `DNS_ZONEFILE_RESOLVER_SNIPPET` ، `--dns-zonefile-resolver-snippet` | حل کرنے والا اسنیپٹ راہ (پہلے سے طے شدہ `ops/soradns/static_zones.<hostname>.json`)۔ |
| `DNS_ZONEFILE_TTL` ، `--dns-zonefile-ttl` | ریکارڈنگ کے لئے ٹی ٹی ایل (پہلے سے طے شدہ: 600 سیکنڈ)۔ |
| `DNS_ZONEFILE_IPV4` ، `--dns-zonefile-ipv4` | IPv4 پتے (کوما سے الگ الگ env یا تکرار کرنے والا جھنڈا)۔ |
| `DNS_ZONEFILE_IPV6` ، `--dns-zonefile-ipv6` | IPv6 پتے۔ |
| `DNS_ZONEFILE_CNAME` ، `--dns-zonefile-cname` | اختیاری CNAME ہدف۔ |
| `DNS_ZONEFILE_SPKI` ، `--dns-zonefile-spki-pin` | SHA-256 SPKI پن (BASE64)۔ |
| `DNS_ZONEFILE_TXT` ، `--dns-zonefile-txt` | اضافی TXT ریکارڈز (`key=value`)۔ |
| `DNS_ZONEFILE_VERSION` ، `--dns-zonefile-version` | کمپیوٹڈ زون فائل ورژن لیبل کے لئے اوور رائڈ۔ |
| `DNS_ZONEFILE_EFFECTIVE_AT` ، `--dns-zonefile-effective-at` | کٹ اوور ونڈو کے آغاز کے بجائے OpenAPI ٹائم اسٹیمپ (RFC3339) کی وضاحت کریں۔ |
| `DNS_ZONEFILE_PROOF` ، `--dns-zonefile-proof` | میٹا ڈیٹا میں پروف لغوی کے لئے اوور رائڈ۔ |
| `DNS_ZONEFILE_CID` ، `--dns-zonefile-cid` | میٹا ڈیٹا میں سی آئی ڈی کے لئے اوور رائڈ۔ |
| `DNS_ZONEFILE_FREEZE_STATE` ، `--dns-zonefile-freeze-state` | گارڈین منجمد ریاست (نرم ، سخت ، پگھلنا ، نگرانی ، ہنگامی صورتحال)۔ |
| `DNS_ZONEFILE_FREEZE_TICKET` ، `--dns-zonefile-freeze-ticket` | منجمد کے لئے گارڈین/کونسل کا ٹکٹ حوالہ۔ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT` ، `--dns-zonefile-freeze-expires-at` | پگھلنے کے لئے RFC3339 ٹائم اسٹیمپ۔ |
| `DNS_ZONEFILE_FREEZE_NOTES` ، `--dns-zonefile-freeze-note` | اضافی منجمد نوٹ (کوما سے الگ الگ اینو یا ریپیٹ ایبل پرچم)۔ |
| `DNS_GAR_DIGEST` ، `--dns-gar-digest` | بلیک 3-256 ڈائجسٹ (ہیکس) نے گار پے لوڈ پر دستخط کیے۔ گیٹ وے بائنڈنگ کے لئے ضروری ہے۔ |

ورک فلو گٹ ہب ایکشنز ان اقدار کو رازوں کے ذخیرے سے پڑھتا ہے تاکہ ہر پروڈکشن پن خود بخود زون فائل نمونے جاری کرے۔ مندرجہ ذیل راز مرتب کریں (لائنوں میں ملٹی ویلیو فیلڈز کے لئے کوما سے الگ الگ فہرستیں شامل ہوسکتی ہیں):

| خفیہ | منزل |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME` ، `DOCS_SORAFS_DNS_ZONE` | ہیلپر کے لئے پروڈکشن میزبان نام/زون۔ |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | ڈسکرپٹر میں آن کال عرف۔ |
| `DOCS_SORAFS_ZONEFILE_IPV4` ، `DOCS_SORAFS_ZONEFILE_IPV6` | اشاعت کے لئے IPv4/IPv6 ریکارڈز۔ |
| `DOCS_SORAFS_ZONEFILE_CNAME` | اختیاری CNAME ہدف۔ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | ایس پی کے آئی پن بیس 64۔ |
| `DOCS_SORAFS_ZONEFILE_TXT` | اضافی TXT ریکارڈز۔ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | کنکال میں میٹا ڈیٹا منجمد کریں۔ |
| `DOCS_SORAFS_GAR_DIGEST` | ہیکس بلیک 3 ڈائجسٹ نے گار پے لوڈ پر دستخط کیے۔ |

جب `.github/workflows/docs-portal-sorafs-pin.yml` چلاتے ہو تو ، ان پٹ `dns_change_ticket` اور `dns_cutover_window` کی وضاحت کریں تاکہ ڈسکرپٹر/زون فائل درست ونڈو میٹا ڈیٹا کو وراثت میں مل جائے۔ انہیں صرف خشک رن کے لئے خالی چھوڑ دیں۔

عام کمانڈ (رن بک SN-7 مالک کے مطابق):

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
```ہیلپر خود بخود تبدیلی کے ٹکٹ کو TXT ریکارڈ میں منتقل کرتا ہے اور کٹور ونڈو کے آغاز کو `effective_at` کے طور پر وراثت میں ملتا ہے ، جب تک کہ دوسری صورت میں اس کی وضاحت نہ کی جائے۔ مکمل آپریشنل ورک فلو کے لئے `docs/source/sorafs_gateway_dns_owner_runbook.md` دیکھیں۔

### عوامی DNS وفد کے بارے میں ایک نوٹ

زون فائل کنکال صرف مستند زون کے اندراجات کی وضاحت کرتا ہے۔ وفد این ایس/ڈی ایس
پیرنٹ زون کو رجسٹرار یا DNS فراہم کنندہ میں تشکیل دینا ضروری ہے تاکہ
باقاعدہ انٹرنیٹ آپ کے ناموروں کو تلاش کرسکتا ہے۔

- اپیکس/ٹی ایل ڈی پر کٹ اوور کے لئے عرف/انیم (فراہم کنندہ پر منحصر ہے) یا استعمال کریں
  کسی بھی کاسٹ-آئی پی گیٹ وے کی طرف اشارہ کرتے ہوئے A/AAAA ریکارڈ شائع کریں۔
- سب ڈومینز کے لئے ، ماخوذ خوبصورت میزبان کو CNAME شائع کریں
  (`<fqdn>.gw.sora.name`)۔
- کیننیکل میزبان (`<hash>.gw.sora.id`) گیٹ وے ڈومین میں رہتا ہے اور ایسا نہیں کرتا ہے
  آپ کے عوامی علاقے میں شائع ہوا۔

### گیٹ وے ہیڈر ٹیمپلیٹ

تعینات کرنے والا مددگار `portal.gateway.headers.txt` اور `portal.gateway.binding.json` بھی تیار کرتا ہے-دو نمونے جو DG-3 گیٹ وے-کانٹے کے پابند کرنے کی ضروریات کو پورا کرتے ہیں:

- `portal.gateway.headers.txt` میں HTTP ہیڈرز کا ایک مکمل بلاک ہے (بشمول `Sora-Name` ، `Sora-Content-CID` ، `Sora-Proof` ، CSP ، HSTS اور `Sora-Route-Binding`) ، جس میں ایج گیٹ وے کو ہر ردعمل سے منسلک ہونا چاہئے۔
- `portal.gateway.binding.json` مشین سے پڑھنے کے قابل شکل میں وہی معلومات حاصل کرتا ہے تاکہ ٹکٹ اور آٹومیشن کو تبدیل کریں شیل آؤٹ پٹ کو پارس کیے بغیر میزبان/سی آئی ڈی بائنڈنگ کا موازنہ کرسکیں۔

وہ `cargo xtask soradns-binding-template` کے ذریعے تیار کیے جاتے ہیں اور عرف ، مینی فیسٹ ڈائجسٹ اور گیٹ وے کے میزبان نام کو حاصل کرتے ہیں جو `sorafs-pin-release.sh` کو منتقل کردیئے گئے تھے۔ ہیڈر بلاک کو دوبارہ تخلیق کرنے یا اپنی مرضی کے مطابق بنانے کے لئے ، چلائیں:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

ہیڈر ٹیمپلیٹس کو اوور رائڈ کرنے کے لئے `--csp-template` ، `--permissions-template` ، یا `--hsts-template` پاس کریں۔ ہیڈر کو مکمل طور پر دور کرنے کے لئے ان کو `--no-*` جھنڈوں کے ساتھ مل کر استعمال کریں۔

سی ڈی این تبدیلی کی درخواست میں ہیڈر کا ایک ٹکڑا منسلک کریں اور جے ایس او این کو گیٹ وے آٹومیشن پائپ لائن پر منتقل کریں تاکہ میزبان کی اصل پیشرفت شواہد سے مماثل ہو۔

ریلیز اسکرپٹ خود بخود توثیق کرنے والا مددگار چلاتا ہے تاکہ DG-3 ٹکٹوں میں ہمیشہ تازہ ترین ثبوت موجود ہوں۔ اگر آپ JSON کو دستی طور پر بائنڈنگ میں ترمیم کرتے ہیں تو دستی طور پر دوبارہ شروع کریں:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

کمانڈ ڈیکوڈس نے `Sora-Proof` کو اسٹپل کیا ، چیک کرتا ہے کہ `Sora-Route-Binding` مینی فیسٹ سی آئی ڈی + ہوسٹ نام سے مماثل ہے اور جب ہیڈر بڑھنے پر ناکام ہوجاتا ہے۔ سی آئی کے باہر چلتے وقت ریلیز کے دیگر نمونے کے ساتھ آؤٹ پٹ کو محفوظ کریں تاکہ ڈی جی 3 جائزہ لینے والوں کے پاس توثیق کا ثبوت ہو۔

> ** ڈی این ایس ڈسکرپٹر انضمام: ** `portal.dns-cutover.json` میں اب ایک سیکشن `gateway_binding` ان نمونے کی طرف اشارہ کیا گیا ہے (راستے ، مواد سی آئی ڈی ، پروف اسٹیٹس اور لغوی ہیڈر ٹیمپلیٹ) ان بلاکس کو ہر DG-3 چینج ٹکٹ میں شامل کریں تاکہ جائزہ لینے والے `Sora-Name/Sora-Proof/CSP` کی صحیح اقدار کا موازنہ کرسکیں اور تعمیر آرکائیو کو کھولے بغیر پروموشن/رول بیک ثبوت کے بنڈل منصوبوں کی تعمیل کی تصدیق کرسکیں۔

## مرحلہ 9 - پبلشنگ مانیٹر لانچ کریںروڈ میپ آئٹم ** دستاویزات -3 سی ** کے لئے جاری ثبوت کی ضرورت ہے کہ پورٹل ، اس کی کوشش کریں پراکسی اور گیٹ وے کی پابندیوں کی رہائی کے بعد صحت مند رہیں۔ 7-8 اقدامات کے فورا. بعد مستحکم مانیٹر لانچ کریں اور اسے اپنی شیڈول تحقیقات سے مربوط کریں:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` بوجھ کنفیگ (اسکیما کے لئے `docs/portal/docs/devportal/publishing-monitoring.md` دیکھیں) اور تین چیک انجام دیتا ہے: پورٹل پاتھ پروبس + سی ایس پی/اجازت-پالیسی کی توثیق ، ​​اس کو پراکسی پراسسی پروسیس (اختیاری `/metrics`) ، اور گیٹ وے بائنڈنگ تصدیق (I18NI) ، اور گیٹ وے بائنڈنگ تصدیق Sora-content-CID کے ساتھ عرف/مینی فیسٹ چیک کے ساتھ۔
- کمانڈ غیر صفر کے ساتھ باہر نکل جاتی ہے جب کوئی تحقیقات ناکام ہوجاتی ہیں ، تاکہ سی آئی/کرون/رن بک آپریٹرز عرف کی تشہیر سے قبل رہائی کو روک سکیں۔
- پرچم `--json-out` اہداف کے لئے ایک واحد خلاصہ JSON لکھتا ہے۔ `--evidence-dir` `summary.json` ، `portal.json` ، `tryit.json` ، `binding.json` اور `checksums.sha256` لکھتا ہے تاکہ گورننس کے جائزہ لینے والے نگرانی کے بغیر دوبارہ چلانے کے نتائج کا موازنہ کرسکیں۔ `artifacts/sorafs/<tag>/monitoring/` کے تحت اس ڈائرکٹری کو Sigstore بنڈل اور DNS کٹ اوور ڈسکرپٹر کے ساتھ محفوظ کریں۔
- ریلیز ٹکٹ میں نگرانی کی آؤٹ پٹ ، Grafana برآمد (`dashboards/grafana/docs_portal.json`) اور الرٹ مینجر ڈرل ID شامل کریں تاکہ بعد میں DOCS-3C SLO کا آڈٹ کیا جاسکے۔ اشاعت کی نگرانی کرنے والی پلے بوک `docs/portal/docs/devportal/publishing-monitoring.md` میں واقع ہے۔

پورٹل پروبس کو HTTPS کی ضرورت ہوتی ہے اور `http://` بیس URL کو مسترد کرتے ہیں جب تک کہ `allowInsecureHttp` مانیٹر کنفیگ میں متعین نہیں کیا جاتا ہے۔ TLS پر پروڈکشن/اسٹیجنگ رکھیں اور صرف مقامی پیش نظاروں کے لئے اوور رائڈ کو قابل بنائیں۔

پورٹل شروع ہونے کے بعد بلڈکائٹ/کرون میں `npm run monitor:publishing` کے ذریعے مانیٹر کو خود کار بنائیں۔ اسی ٹیم نے پروڈکشن یو آر ایل ایس پر توجہ مرکوز کی جس میں ریلیز کے مابین صحت کی مستقل جانچ پڑتال ہوتی ہے۔

## `sorafs-pin-release.sh` کے ذریعے آٹومیشن

`docs/portal/scripts/sorafs-pin-release.sh` اقدامات 2-6 سے منسلک کرتا ہے۔ وہ:

1. آرکائیوز `build/` ایک ڈٹرمینسٹک ٹربال میں ،
2. لانچ `car pack` ، `manifest build` ، `manifest sign` ، `manifest verify-signature` اور `proof verify` ،
3. اختیاری طور پر `manifest submit` پر عمل درآمد ہوتا ہے (بشمول عرف بائنڈنگ) اگر Torii اسناد دستیاب ہوں ، اور
4. `artifacts/sorafs/portal.pin.report.json` لکھتے ہیں ، اختیاری `portal.pin.proposal.json` ، DNS کٹ اوور ڈسکرپٹر (جمع کرانے کے بعد) اور گیٹ وے بائنڈنگ بنڈل (`portal.gateway.binding.json` پلس ہیڈر بلاک) تاکہ گورننس/نیٹ ورکنگ/او پی ایس ٹیمیں CI لاگوں کا مطالعہ کیے بغیر شواہد کی تصدیق کرسکیں۔

شروع کرنے سے پہلے ، `PIN_ALIAS` ، `PIN_ALIAS_NAMESPACE` ، `PIN_ALIAS_NAME` اور (اختیاری) `PIN_ALIAS_PROOF_PATH` انسٹال کریں۔ خشک رن کے لئے `--skip-submit` استعمال کریں۔ ذیل میں گٹ ہب ورک فلو ان پٹ `perform_submit` کے ذریعے ٹوگل کرتا ہے۔

## مرحلہ 8 - اشاعت OpenAPI چشمی اور SBOM بنڈل

DOCS-7 کا تقاضا ہے کہ پورٹل ، OpenAPI SPECT اور SBOM نمونے اسی طرح کی پائپ لائن سے گزرتے ہیں۔ دستیاب مددگار تینوں کا احاطہ کرتے ہیں:

1. ** دوبارہ تخلیق کریں اور سائن کریں۔ **

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```تاریخی اسنیپ شاٹ کو بچانے کے لئے `--version=<label>` کی وضاحت کریں (مثال کے طور پر ، `2025-q3`)۔ ہیلپر `static/openapi/versions/<label>/torii.json` پر اسنیپ شاٹ لکھتا ہے ، `versions/current` پر آئینہ دیتا ہے اور میٹا ڈیٹا (SHA-256 ، مینی فیسٹ اسٹیٹس ، تازہ ترین ٹائم اسٹیمپ) کو `static/openapi/versions.json` پر لکھتا ہے۔ پورٹل اس انڈیکس کا استعمال ورژن کو منتخب کرنے اور ڈائجسٹ/دستخط کو ظاہر کرنے کے لئے کرتا ہے۔ اگر `--version` کی وضاحت نہیں کی گئی ہے تو ، موجودہ لیبل برقرار رکھے گئے ہیں اور صرف `current` + `latest` کو اپ ڈیٹ کیا گیا ہے۔

   منشور SHA-256/BLAKE3 ہضموں کو ٹھیک کرتا ہے تاکہ گیٹ وے `Sora-Proof` کو `/reference/torii-swagger` پر قائم رکھ سکے۔

2. ** cyclonedx sboms تیار کریں۔ نمونے بنانے کے لئے آؤٹ پٹ کو آگے رکھیں:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. ** ہر پے لوڈ کو کار میں پیک کریں۔ **

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

   اسی اقدامات پر عمل کریں `manifest build`/Sigstore مرکزی سائٹ کے طور پر ، نمونے کے لئے الگ الگ عرفات کی وضاحت کریں (مثال کے طور پر ، spec کے لئے `docs-openapi.sora` اور دستخط شدہ SBOM کے لئے `docs-sbom.sora`)۔ انفرادی عرفی نام ایک مخصوص پے لوڈ سے بندھے ہوئے سورادنس ، گار اور رول بیک ٹکٹ چھوڑ دیتے ہیں۔

4. ** جمع کروائیں اور پابند کریں۔

پورٹل بلڈ کے ساتھ آرکائیو اسپیشل/ایس بی او ایم ظاہر ہوتا ہے اس بات کو یقینی بناتا ہے کہ ہر ریلیز کے ٹکٹ میں پیکر کو دوبارہ چلائے بغیر نمونے کا ایک مکمل سیٹ شامل ہے۔

### آٹومیشن ہیلپر (CI/پیکیج اسکرپٹ)

`./ci/package_docs_portal_sorafs.sh` قدم 1-8 کو انکوڈ کرتا ہے تاکہ روڈ میپ آئٹم ** دستاویزات -7 ** کو ایک کمانڈ میں عمل میں لایا جاسکے۔ مددگار:

- پورٹل (`npm ci` ، OpenAPI/Norito مطابقت پذیری ، ویجیٹ ٹیسٹ) تیار کرتا ہے۔
- `sorafs_cli` کے ذریعے پورٹل ، OpenAPI اور SBOM کے لئے کاریں اور ظاہر جوڑے تیار کرتا ہے۔
- اختیاری طور پر `sorafs_cli proof verify` (`--proof`) اور Sigstore سائننگ (`--sign` ، `--sigstore-provider` ، `--sigstore-audience`) انجام دیتا ہے۔
- `artifacts/devportal/sorafs/<timestamp>/` میں تمام نمونے ڈالتا ہے اور CI/ریلیز ٹولنگ کے لئے `package_summary.json` لکھتا ہے۔ اور
- آخری رن میں `artifacts/devportal/sorafs/latest` کو اپ ڈیٹ کریں۔

مثال (Sigstore + POR کے ساتھ مکمل پائپ لائن):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

مفید جھنڈے:

- `--out <dir>` - نمونے کی جڑ کو اوور رائڈ (بطور ڈیفالٹ ٹائم اسٹیمپ کے ساتھ ڈائریکٹریز)۔
- `--skip-build` - موجودہ `docs/portal/build` (آف لائن آئینے کے لئے مفید) استعمال کریں۔
- `--skip-sync-openapi` - `npm run sync-openapi` کو چھوڑیں جب `cargo xtask openapi` کریٹ.یو تک نہیں پہنچ سکتا۔
- `--skip-sbom` - اگر بائنری انسٹال نہیں کیا گیا ہے تو `syft` پر کال نہ کریں (اسکرپٹ ایک انتباہ پرنٹ کرتا ہے)۔
- `--proof` - ہر کار/منشور جوڑی کے لئے `sorafs_cli proof verify` پر عمل کریں۔ ملٹی فائل پے لوڈ کو فی الحال سی ایل آئی میں CHUNK-PLAN سپورٹ کی ضرورت ہوتی ہے ، لہذا اس جھنڈے کو `plan chunk count` غلطیوں کے لئے غیر فعال چھوڑ دیں اور گیٹ کے ظاہر ہونے کے بعد دستی طور پر چیک کریں۔
- `--sign` - `sorafs_cli manifest sign` پر کال کریں۔ `SIGSTORE_ID_TOKEN` (یا `--sigstore-token-env`) کے ذریعے ٹوکن پاس کریں یا CLI کو `--sigstore-provider/--sigstore-audience` کے ذریعے بازیافت کرنے دیں۔پروڈکشن نمونے کے لئے `docs/portal/scripts/sorafs-pin-release.sh` استعمال کریں۔ یہ پورٹل ، OpenAPI اور SBOM پیکج کرتا ہے ، ہر مینی فیسٹ پر دستخط کرتا ہے اور `portal.additional_assets.json` پر اضافی میٹا ڈیٹا لکھتا ہے۔ ہیلپر اسی طرح کے نوبس کو سمجھتا ہے جیسے سی آئی پیکجر ، نیز `--openapi-*` ، `--portal-sbom-*` اور `--openapi-sbom-*` `--openapi-sbom-source` کے ذریعے عرف ٹوپل ، اوور رائڈ ایس بی او ایم ماخذ کے لئے ، `--openapi-sbom-source` (`--openapi-sbom-source` کے ذریعے اوور رائڈ ایس بی او ایم سورس کو اوور رائڈ `--syft-bin` کے ذریعے غیر معیاری `syft`۔

اسکرپٹ تمام احکامات دکھاتا ہے۔ `package_summary.json` کے ساتھ ساتھ لاگ کو ریلیز کے ٹکٹ میں کاپی کریں تاکہ جائزہ لینے والے کار ڈائجسٹ کا موازنہ کرسکیں ، میٹا ڈیٹا اور Sigstore بنڈل ہیشوں کو بے ترتیب شیل آؤٹ پٹ کو دیکھے بغیر۔

## مرحلہ 9 - گیٹ وے + سورڈن چیک کریں

کٹ اوور کا اعلان کرنے سے پہلے ، اس بات کو یقینی بنائیں کہ نیا عرف سوردنز کے ذریعہ حل ہوا ہے اور گیٹ ویز تازہ ثبوتوں کو چسپاں کرتے ہیں:

1. ** تحقیقات گیٹ چلائیں۔ حقیقی تعیناتیوں کے لئے ، ہدف کے میزبان نام کی وضاحت کریں:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   تحقیقات ڈیکوڈز `Sora-Name` ، `Sora-Proof` اور `Sora-Proof-Status` by `docs/source/sorafs_alias_policy.md` اور جب مینی فیسٹ ڈائجسٹ ، TTL یا GAR بائنڈنگز ڈائیورج ہوتے ہیں تو کریش ہوتا ہے۔

   ہلکا پھلکا اسپاٹ چیک کے لئے (مثال کے طور پر ، جب صرف پابند بنڈل
   تبدیل شدہ) ، `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>` چلائیں۔
   مددگار پکڑے گئے بائنڈنگ بنڈل کی توثیق کرتا ہے اور رہائی کے لئے آسان ہے
   وہ ٹکٹ جن کو صرف مکمل تحقیقات کی مشق کی بجائے پابند تصدیق کی ضرورت ہوتی ہے۔

2. ** ڈرل کے لئے ثبوت جمع کریں۔ ریپر `artifacts/sorafs_gateway_probe/<stamp>/` ، `ops/drill-log.md` کو اپ ڈیٹ کرتا ہے اور اختیاری طور پر رول بیک ہکس یا پیجریڈی پے لوڈ کو چلاتا ہے۔ IP نہیں بلکہ سورادنس کے راستے کی جانچ پڑتال کے لئے `--host docs.sora` سیٹ کریں۔

3. ** ڈی این ایس بائنڈنگز کو چیک کریں۔ حل کرنے والے مالکان `tools/soradns-resolver` کے ذریعے ایک ہی ان پٹ چلا سکتے ہیں تاکہ یہ یقینی بنایا جاسکے کہ کیچڈ اندراجات نئے مینی فیسٹ سے مماثل ہیں۔ JSON منسلک کرنے سے پہلے ، `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]` چلائیں تاکہ ڈٹرمینسٹک میزبان میپنگ ، مینی فیسٹ میٹا ڈیٹا ، اور ٹیلی میٹری لیبل کو آف لائن کی توثیق کی جائے۔ مددگار دستخط شدہ گار کے ساتھ ہی `--json-out` خلاصہ پیدا کرسکتا ہے۔
  نیا GAR تیار کرتے وقت ، `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...` استعمال کریں (اگر کوئی مینی فیسٹ فائل نہ ہو تو صرف `--manifest-cid <cid>` میں واپس جائیں)۔ ہیلپر اب سی آئی ڈی ** اور بلیک 3 ڈائجسٹ کو براہ راست منشور JSON سے ہضم کرتا ہے ، خالی جگہوں کو ہٹا دیتا ہے ، ڈپلیکیٹ `--telemetry-label` ، ترتیب دیتا ہے اور JSON پیدا کرنے سے پہلے پہلے سے طے شدہ CSP/HSTS/اجازت-پولیسی ٹیمپلیٹس کو لکھتا ہے تاکہ پے لوڈ کا تعی .ن ہوجائے۔

4. ** عرفی میٹرکس کی نگرانی کریں۔ ** `torii_sorafs_alias_cache_refresh_duration_ms` اور `torii_sorafs_gateway_refusals_total{profile="docs"}` کو اسکرین پر رکھیں۔ دونوں سیریز `dashboards/grafana/docs_portal.json` میں ہیں۔

## مرحلہ 10 - نگرانی اور شواہد بنڈلنگ- ** ڈیش بورڈز۔ JSON ایکسپورٹ کو ریلیز ٹکٹ سے منسلک کریں۔
- ** تحقیقات آرکائیوز۔ تحقیقات کا خلاصہ ، ہیڈر اور پیجریڈی پے لوڈ شامل کریں۔
-** ریلیز بنڈل۔
- ** ڈرل لاگ۔
- ** ٹکٹ کے لنکس۔

## مرحلہ 11 - ملٹی سورس بازیافت ڈرل اور اسکور بورڈ ثبوت

SoraFS پر اشاعت کے لئے اب DNS/گیٹ وے ثبوت کے ساتھ ملٹی سورس بازیافت ثبوت (DOCS-7/SF-6) کی ضرورت ہے۔ ظاہر پن کے بعد:

1. ** براہ راست مینی فیسٹ پر `sorafs_fetch` چلائیں۔ تمام آؤٹ پٹ کو محفوظ کریں تاکہ آڈیٹر آرکسٹریٹر کے حل کی نقل تیار کرسکیں:

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

   - پہلے مینی فیسٹ (جیسے `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) میں فراہم کنندہ کے اشتہارات حاصل کریں اور ان کو `--provider-advert name=path` کے ذریعے منتقل کریں تاکہ اسکور بورڈ مواقع کی کھڑکیوں کا تعی .ن سے اندازہ کرے۔ `--allow-implicit-provider-metadata` استعمال کریں ** صرف ** CI فکسچر کے لئے ؛ پروڈکشن ڈرل کو پن سے دستخط شدہ اشتہارات سے لنک کرنا چاہئے۔
   اگر اضافی خطے موجود ہیں تو ، مناسب فراہم کنندہ ٹیپلس کے ساتھ کمانڈ کو دہرائیں تاکہ ہر کیشے/عرف سے وابستہ بازیافت کا نمونہ ہو۔

2. ** محفوظ شدہ دستاویزات۔ یہ فائلیں SF-7 کے لئے ہم مرتبہ وزن ، دوبارہ کوشش کرنے والا بجٹ ، EWMA لیٹینسی اور حصہ رسیدیں ریکارڈ کرتی ہیں۔

3. ** ٹیلی میٹری کو اپ ڈیٹ کریں۔ Grafana اسنیپ شاٹس کو ریلیز ٹکٹ کے ساتھ اسکور بورڈ کے راستے کے ساتھ منسلک کریں۔

4. ** الرٹ کے قواعد چیک کریں۔ پرومٹول آؤٹ پٹ منسلک کریں تاکہ DOCS-7 جائزہ لینے والے اس بات کی تصدیق کریں کہ اسٹال/سست فراہم کرنے والے انتباہات فعال ہیں۔

5. ** CI میں جڑیں۔ اسے اسٹیج/پروڈکشن کے ل enable قابل بنائیں تاکہ منشور کے بنڈل کے ساتھ ساتھ بازیافت کے ثبوت بھی پیدا ہوں۔ مقامی مشقیں وہی اسکرپٹ استعمال کرسکتی ہیں ، گیٹ وے ٹوکن کو برآمد کر سکتی ہیں اور `PIN_FETCH_PROVIDERS` (فراہم کنندگان کی کوما سے الگ الگ فہرست) ترتیب دے سکتی ہیں۔

## تشہیر ، مشاہدہ اور رول بیک1. ** پروموشن: ** الگ الگ اسٹیجنگ اور پروڈکشن عرف رکھیں۔ اسی مینی فیسٹ/بنڈل کے ساتھ `manifest submit` کو دوبارہ چلانے کے ذریعہ فروغ دیں ، `--alias-namespace/--alias-name` کو پروڈکشن عرف میں تبدیل کریں۔ اس میں QA کی منظوری کے بعد دوبارہ تعمیر/دوبارہ دستخط کرنے کو خارج نہیں کیا گیا ہے۔
2. ** مانیٹرنگ: ** ڈیش بورڈ پن ریگسٹری (`docs/source/grafana_sorafs_pin_registry.json`) اور پورٹل پروبس (`docs/portal/docs/devportal/observability.md`) سے رابطہ کریں۔ چیکسم بڑھے ہوئے ، ناکام تحقیقات یا اسپائکس کی دوبارہ کوشش کرنے کے لئے نگاہ رکھیں۔
3. ** رول بیک: ** رول بیک کے لئے ، `sorafs_cli manifest submit --alias ... --retire` کے ساتھ پچھلے مینی فیسٹ (یا موجودہ عرف کو ریٹائر ہونے) کو دوبارہ جاری کریں۔ تازہ ترین معروف گڈ بنڈل اور کار کا خلاصہ ہمیشہ رکھیں تاکہ سی آئی لاگز کو گھومتے وقت رول بیک پروف کو دوبارہ پیش کیا جاسکے۔

## CI ورک فلو ٹیمپلیٹ

کم سے کم پائپ لائن چاہئے:

1. بلڈ + لنٹ (`npm ci` ، `npm run build` ، چیکسم جنریشن)۔
2. پیکیجنگ (`car pack`) اور حساب کتاب ظاہر ہوتا ہے۔
3. جاب اسکوپڈ OIDC ٹوکن (`manifest sign`) کے ذریعے دستخط۔
4. آڈیٹنگ کے لئے نمونے (کار ، منشور ، بنڈل ، منصوبہ ، خلاصے) کو لوڈ کرنا۔
5. پن رجسٹری کو بھیجنا:
   - درخواستیں کھینچیں -> `docs-preview.sora`۔
   - ٹیگز / محفوظ شاخیں -> پروموشن پروڈکشن عرف۔
6. تکمیل سے پہلے تحقیقات + پروف تصدیق کے دروازے چلائیں۔

`.github/workflows/docs-portal-sorafs-pin.yml` ان مراحل کو دستی ریلیز کے لئے جوڑتا ہے۔ ورک فلو:

- بلڈ/ٹیسٹ پورٹل ،
- `scripts/sorafs-pin-release.sh` کے ذریعے پیکیجنگ بلڈ ،
- Github OIDC کے ذریعے منشور بنڈل کی دستخط/توثیق ،
- لوڈنگ کار/منشور/بنڈل/پلان/پروف خلاصے بطور نمونے ، اور
- (اختیاری) اگر راز دستیاب ہوں تو منشور + عرف بائنڈنگ بھیجنا۔

نوکری چلانے سے پہلے درج ذیل ذخیرہ راز/متغیرات مرتب کریں:

| نام | منزل |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Torii `/v2/sorafs/pin/register` کے ساتھ میزبان۔ |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | ایپچ شناخت کنندہ جمع کرانے کے دوران لکھا گیا۔ |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | ظاہر جمع کروانے کے لئے دستخطی اتھارٹی۔ |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | عرف ٹوپل ، پابند جب `perform_submit` = `true`۔ |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | بیس 64 عرف پروف بنڈل (اختیاری)۔ |
| `DOCS_ANALYTICS_*` | موجودہ تجزیات/تحقیقات کے اختتامی نکات۔ |

ایکشن UI کے ذریعے ورک فلو لانچ کریں:

1. `alias_label` کی وضاحت کریں (مثال کے طور پر ، `docs.sora.link`) ، اختیاری `proposal_alias` اور اختیاری `release_tag`۔
2. `perform_submit` کو Torii (ڈرائی رن) کے بغیر نمونے تیار کرنے کے لئے غیر فعال چھوڑ دیں یا اسے تشکیل شدہ عرف میں شائع کرنے کے قابل بنائیں۔

`docs/source/sorafs_ci_templates.md` بیرونی منصوبوں کے لئے عام سی آئی مددگاروں کی وضاحت کرتا ہے ، لیکن روزمرہ کی رہائی کے ل you آپ کو پورٹل ورک فلو استعمال کرنا چاہئے۔

## چیک لسٹ- [] `npm run build` ، `npm run test:*` اور `npm run check:links` پاس۔
- [] `build/checksums.sha256` اور `build/release.json` نمونے میں محفوظ ہیں۔
- [] کار ، منصوبہ ، ظاہر اور خلاصہ `artifacts/` کے تحت تیار کیا گیا۔
- [] Sigstore بنڈل + علیحدہ دستخط لاگ کے ساتھ محفوظ کردہ۔
- [] `portal.manifest.submit.summary.json` اور `portal.manifest.submit.response.json` جمع کرانے کے بعد محفوظ ہوگئے ہیں۔
- [] `portal.pin.report.json` (اور اختیاری `portal.pin.proposal.json`) کار/ظاہر نمونے کے ساتھ ہی محفوظ شدہ دستاویزات ہیں۔
- [] لاگز `proof verify` اور `manifest verify-signature` محفوظ ہیں۔
- [] Grafana ڈیش بورڈز اپ ڈیٹ + کوشش کریں- یہ تحقیقات کامیاب ہیں۔
- [] رول بیک نوٹس (پچھلے مینی فیسٹ + عرف ڈائجسٹ کی شناخت) ریلیز ٹکٹ کے ساتھ منسلک ہیں۔