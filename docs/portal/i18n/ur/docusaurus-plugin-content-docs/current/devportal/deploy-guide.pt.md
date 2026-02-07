---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/deploy-guide.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## جائزہ

یہ پلے بک روڈ میپ آئٹمز ** دستاویزات -7 ** (SoraFS کے ذریعہ شائع کردہ) اور ** دستاویزات -8 ** کو تبدیل کرتی ہے۔
(CI/CD پن آٹومیشن) ڈویلپر پورٹل کے لئے قابل عمل طریقہ کار میں۔
بلڈ/لنٹ مرحلے ، پیکیجنگ SoraFS کا احاطہ کرتا ہے ، Sigstore کے ساتھ ظاہر ہوتا ہے ،
عرف پروموشن ، توثیق ، ​​اور رول بیک مشقیں تاکہ ہر پیش نظارہ اور ریلیز
تولیدی اور قابل آڈٹیبل بنیں۔

بہاؤ فرض کرتا ہے کہ آپ کے پاس بائنری `sorafs_cli` ہے (`--features cli` کے ساتھ بنایا گیا ہے) ، تک رسائی
Torii PIN-REGISTIRY اجازت کے ساتھ اختتامی نقطہ ، اور OIDC اسناد Sigstore کے لئے۔ راز رکھیں
آپ کے CI والٹ میں طویل عرصے تک (`IROHA_PRIVATE_KEY` ، `SIGSTORE_ID_TOKEN` ، Torii ٹوکن) ؛
مقامی رنز انہیں شیل برآمدات سے لوڈ کرسکتے ہیں۔

## شرائط

- `npm` یا `pnpm` کے ساتھ نوڈ 18.18+۔
- `sorafs_cli` `cargo run -p sorafs_car --features cli --bin sorafs_cli` سے۔
- Torii کا URL جو `/v1/sorafs/*` کے علاوہ ایک اتھارٹی اکاؤنٹ/نجی کلید کو بے نقاب کرتا ہے جو ظاہر اور عرفی نام بھیج سکتا ہے۔
- `SIGSTORE_ID_TOKEN` جاری کرنے کے لئے جاری کرنے والا OIDC (گٹ ہب ایکشنز ، گٹ لیب ، کام کے بوجھ کی شناخت ، وغیرہ)۔
- اختیاری: `examples/sorafs_cli_quickstart.sh` DRE رنز کے لئے اور `docs/source/sorafs_ci_templates.md` کو گٹھب/گٹ لیب ورک فلوز کے لئے سہاروں کے لئے۔
- اس کی تشکیل کریں
 [سیکیورٹی سخت کرنے والی چیک لسٹ] (./security-hardening.md) کسی تعمیر کو فروغ دینے سے پہلے
 لیب سے باہر جب یہ متغیرات غائب ہوں گے تو پورٹل بلڈ اب ناکام ہوجاتی ہے
 o جب ٹی ٹی ایل/پولنگ نوبس اطلاق شدہ وینٹوں سے باہر گرتے ہیں۔ برآمد
 `DOCS_OAUTH_ALLOW_INSECURE=1` صرف دستیاب مقامی پیش نظارہ کے لئے۔ منسلک
 رہائی کے ٹکٹ کو قلم کی جانچ کرنے کا ثبوت۔

## مرحلہ 0 - ٹری سے ایک پیکیج پر قبضہ کریں

نیٹ لائف یا گیٹ وے کے پیش نظارہ کو فروغ دینے سے پہلے ، اگلے ٹری کے ذرائع کو چیک کریں اور
منشور OpenAPI کا ڈائجسٹ ایک ڈٹرمینسٹک پیکٹ میں دستخط شدہ:

```bash
cd docs/portal
npm run release:tryit-proxe -- \
 --out ../../artifacts/tryit-proxy/$(date -u +%E%m%dT%H%M%SZ) \
 --target https://torii.dev.sora \
 --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` کاپی پراکسی/تحقیقات/رول بیک مددگار ، دستخط چیک کریں
OpenAPI اور `release.json` لکھیں لیکن `checksums.sha256`۔ اس پیکیج کو ٹکٹ سے منسلک کریں
نیٹ لیف/SoraFS گیٹ وے کی تشہیر تاکہ جائزہ لینے والے عین مطابق ذرائع کو دوبارہ پیش کرسکیں
ڈیل تحقیقات اور بغیر کسی تعمیر نو کے ہدف Torii کے سراگ۔ پیکٹ بھی ریکارڈ کرتا ہے
منصوبے کو برقرار رکھنے کے لئے گاہک کے ذریعہ فراہم کردہ بیئرز فعال ہیں (`allow_client_auth`)
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
- `build/release.json` - میٹا ڈیٹا (`tag` ، `generated_at` ، `source`) ہر کار/منشور میں سیٹ کریں۔

دونوں فائلوں کو کار سمری کے ساتھ محفوظ کریں تاکہ جائزہ لینے والے ڈیزائن نمونے کا موازنہ کرسکیں۔
تعمیر نو کے بغیر پیش نظارہ۔

## مرحلہ 2 - جامد اثاثوں کو پیک کریںDocusaurus کی آؤٹ پٹ ڈائرکٹری کے خلاف کار پیکجر چلائیں۔ اباجو مثال
`artifacts/devportal/` کے نیچے تمام نمونے لکھیں۔

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

JSON ڈائجسٹ نے ٹکڑوں ، ہضموں اور پروف پلاننگ ٹریک کے مندرجات کو اپنی گرفت میں لیا ہے
`manifest build` اور بعد میں CI ڈیش بورڈز کو دوبارہ استعمال کریں۔

## مرحلہ 2B - پیکیجنگ ساتھی OpenAPI اور SBOM

DOCS-7 کے لئے پورٹل سائٹ ، اسنیپ شاٹ OpenAPI اور SBOM پے لوڈ کی اشاعت کی ضرورت ہے
جیسا کہ الگ الگ ظاہر ہوتا ہے تاکہ گیٹ وے ہیڈر کو لپیٹ سکے
`Sora-Proof`/`Sora-Content-CID` ہر نمونے کے لئے۔ رہائی مددگار
(`scripts/sorafs-pin-release.sh`) ڈائریکٹری OpenAPI پیک کرتا ہے
(`static/openapi/`) اور SBOMs الگ کاروں میں `syft` کے ذریعے جاری کیا
`openapi.*`/`*-sbom.*` اور میٹا ڈیٹا کو ریکارڈ کرتا ہے
`artifacts/sorafs/portal.additional_assets.json`۔ دس دستی بہاؤ چلائیں ،
ہر پے لوڈ کے لئے اپنے سابقہ ​​اور میٹا ڈیٹا ٹیگ کے ساتھ 2-4 اقدامات دہرائیں
(جیسے `--car-out "$OUT"/openapi.car` لیکن
`--metadata alias_label=docs.sora.link/openapi`)۔ ہر ظاہر/عرف جوڑی کو ریکارڈ کرتا ہے
DNS کو تبدیل کرنے سے پہلے Torii (سائٹ ، OpenAPI ، پورٹل کا SBOM ، OpenAPI کا SBOM)
گیٹ وے تمام شائع شدہ نمونے کے لئے ایمبیڈڈ ثبوت پیش کرسکتا ہے۔

## مرحلہ 3 - منشور بنائیں

```bash
sorafs_cli manifest build \
 --summare "$OUT"/portal.car.json \
 --manifest-out "$OUT"/portal.manifest.to \
 --manifest-json-out "$OUT"/portal.manifest.json \
 --pin-min-replicas 5 \
 --pin-storage-class warm \
 --pin-retention-epoch 14 \
 --metadata alias_label=docs.sora.link
```

اپنی ریلیز ونڈو کے مطابق پن پالیسی کے جھنڈے مرتب کریں (جیسے `-پرین اسٹوریج کلاس
کینریوں کے لئے گرم)۔ JSON مختلف حالت اختیاری ہے لیکن کوڈ کے جائزے کے لئے آسان ہے۔

## مرحلہ 4 - Sigstore کے ساتھ دستخط کرنا

```bash
sorafs_cli manifest sign \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --bundle-out "$OUT"/portal.manifest.bundle.json \
 --signature-out "$OUT"/portal.manifest.sig \
 --identity-token-provider github-actions \
 --identity-token-audience sorafs-devportal
```

بنڈل نے منشور ڈائجسٹ ، حصہ ڈائجسٹس اور ٹوکن کا ایک بلیک 3 ہیش ریکارڈ کیا ہے
OIDC JWT کو برقرار رکھے بغیر۔ بنڈل اور سبسکرپشن دونوں کو الگ سے محفوظ کریں۔ کی پروموشنز
پروڈکشن دوبارہ دستخط کرنے کے بجائے وہی نمونے دوبارہ استعمال کرسکتا ہے۔ مقامی پھانسی
فراہم کنندہ کے جھنڈوں کو `--identity-token-env` (یا قائم کرنے کے ساتھ تبدیل کرسکتے ہیں
`SIGSTORE_ID_TOKEN` آس پاس کے علاقے میں) جب بیرونی OIDC مددگار ٹوکن جاری کرتا ہے۔

## مرحلہ 5 - پن رجسٹری میں جمع کروائیں

Torii پر دستخط شدہ منشور (اور حصہ منصوبہ) بھیجیں۔ ہمیشہ ایک سمری کی درخواست کریں تاکہ
نتیجے میں داخلہ/عرف انتہائی قابل عمل ہے۔

```bash
sorafs_cli manifest submit \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --torii-url "$TORII_URL" \
 --authorite ih58... \
 --private-kee "$IROHA_PRIVATE_KEY" \
 --submitted-epoch 20260101 \
 --alias-namespace docs \
 --alias-name sora.link \
 --alias-proof "$OUT"/docs.alias.proof \
 --summary-out "$OUT"/portal.submit.json \
 --response-out "$OUT"/portal.submit.response.json
```

جب ایک پیش نظارہ یا کینائر عرف (`docs-preview.sora`) غیر منحصر ہے تو ، دہرائیں
ایک انوکھا عرف کے ساتھ پیش کرنا تاکہ QA کو فروغ دینے سے پہلے مواد کی تصدیق ہوسکے
پیداوار

عرف بائنڈنگ کے لئے تین فیلڈز کی ضرورت ہے: `--alias-namespace` ، `--alias-name` اور `--alias-proof`۔
جب درخواست مطمئن ہوجاتی ہے تو گورننس پروف بنڈل (بیس 64 یا بائٹس Norito) تیار کرتا ہے
ڈیل عرف ؛ اسے CI رازوں میں محفوظ کریں اور `manifest submit` کی درخواست کرنے سے پہلے اسے فائل کے طور پر بے نقاب کریں۔
جب آپ صرف DNS کو چھوئے بغیر منشور کو ترتیب دینے کے بارے میں سوچتے ہیں تو عرف پرچموں کو غیر سیٹ چھوڑ دیں۔

## مرحلہ 5B - گورننس کی تجویز تیار کریں

ہر منشور کو پارلیمنٹ میں مجوزہ فہرست کے ساتھ سفر کرنا چاہئے تاکہ کوئی بھی شہری
سورہ مراعات یافتہ اسناد طلب کیے بغیر تبادلہ داخل کرسکتا ہے۔ قدموں کے بعد
جمع کروائیں/دستخط کریں ، عملدرآمد کریں:

```bash
sorafs_cli manifest proposal \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --submitted-epoch 20260101 \
 --alias-hint docs.sora.link \
 --proposal-out "$OUT"/portal.pin.proposal.json
````portal.pin.proposal.json` نے کیننیکل انسٹرکشن `RegisterPinManifest` کو اپنی گرفت میں لے لیا ،
ہاضمہ ، پالیسی اور عرف سے باخبر رہنا۔ گورننس ٹکٹ کا معاون یا
پارلیمنٹ کا پورٹل تاکہ مندوبین نمونے کی تشکیل نو کے بغیر پے لوڈ کا موازنہ کرسکیں۔
چونکہ کمانڈ کبھی بھی Torii کی اتھارٹی کی کلید کو نہیں چھوتا ہے ، کوئی بھی شہری لکھ سکتا ہے
مقامی طور پر تجویز کیا گیا۔

## مرحلہ 6 - ثبوت اور ٹیلی میٹری چیک کریں

فکسنگ کے بعد ، عزم کی توثیق کے اقدامات انجام دیں:

```bash
sorafs_cli proof verife \
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
 نئے قائم کردہ مواد کے خلاف۔
- بیان کردہ نگرانی کے شواہد کو اپنی گرفت میں لے لیتا ہے
 [پبلشنگ اینڈ مانیٹرنگ] (./publishing-monitoring.md) دستاویزات -3 سی مشاہدہ گیٹ کے لئے
 اشاعت کے مراحل سے مطمئن ہے۔ مددگار اب متعدد آدانوں کو قبول کرتا ہے
 `bindings` (سائٹ ، OpenAPI ، پورٹل کا SBOM ، OpenAPI کا SBOM) اور `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` کا اطلاق ہوتا ہے
 اختیاری گارڈ `hostname` کے ذریعے ہدف کے میزبان پر۔ اباجو کی درخواست دونوں لکھتی ہے
 ثبوت کے بنڈل کے طور پر سنگل JSON خلاصہ (`portal.json` ، `tryit.json` ، `binding.json` اور
 `checksums.sha256`) ریلیز ڈائرکٹری کے تحت:

 ```bash
 npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
 --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
 ```

## مرحلہ 6A - گیٹ وے کے سرٹیفکیٹ کی منصوبہ بندی کریں

گار پیکٹ بنانے سے پہلے SAN/TLS چیلنج پلان حاصل کریں تاکہ گیٹ وے ٹیم اور
ڈی این ایس کے منظوری ایک ہی ثبوت کا جائزہ لیتے ہیں۔ نیا مددگار آٹومیشن آدانوں کی عکاسی کرتا ہے
ڈی جی 3 کی گنتی کیننیکل وائلڈ کارڈ کے میزبان ، خوبصورت میزبان سینز ، DNS-01 ٹیگز ، اور تجویز کردہ ACME چیلنجز:

```bash
cargo xtask soradns-acme-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON کو ریلیز کے بنڈل (ایکسچینج ٹکٹ کے ساتھ ذیلی لنک) کے ساتھ مل کر ارتکاب کریں تاکہ آپریٹرز
Torii اور جائزہ لینے والوں سے تشکیل `torii.sorafs_gateway.acme` میں SAN اقدار حاصل کرسکتے ہیں۔
GAR میزبان اخذات کو دوبارہ چلائے بغیر کیننیکل/پریٹ نقشوں کی تصدیق کرسکتا ہے۔ اجتماعی
اضافی `--name` ہر لاحقہ کے لئے اسی ریلیز میں فروغ پائے جاتے ہیں۔

## مرحلہ 6 بی - کیننیکل میزبان نقشے اخذ کریں

GAR پے لوڈ کو ٹیمپلیٹ کرنے سے پہلے ، ہر عرف کے لئے ڈٹرمینسٹک میزبان میپنگ کو رجسٹر کریں۔
`cargo xtask soradns-hosts` ہر `--name` اس کے کیننیکل ٹیگ میں ہیش کرتا ہے
۔
(`<alias>.gw.sora.name`)۔ ریلیز نمونے میں پیداوار برقرار رہتی ہے تاکہ جائزہ لینے والے
DG-3 نقشہ کا موازنہ GAR بھیجیں کے ساتھ کرسکتا ہے:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json
```

جب گار JSON گیٹ وے کا پابند ہوتا ہے تو جلدی سے ناکام ہونے کے لئے `--verify-host-patterns <file>` استعمال کریں
مطلوبہ میزبانوں میں سے ایک کو چھوڑ دیتا ہے۔ مددگار متعدد توثیق فائلوں کو قبول کرتا ہے
گار اسپریڈشیٹ اور `portal.gateway.binding.json` دونوں کا آسان لنٹ اسی دعوت نامے میں سرایت کیا گیا:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json \
 --verify-host-patterns artifacts/sorafs/portal.gar.json \
 --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```JSON کا خلاصہ اور تصدیق لاگ ان DNS/گیٹ وے تبدیلی کے ٹکٹ پر منسلک کریں تاکہ
آڈیٹر اسکرپٹ کو دوبارہ عمل کیے بغیر کیننیکل ، وائلڈ کارڈ اور پریٹی میزبانوں کی تصدیق کرسکتے ہیں۔
کمانڈ کو دوبارہ عمل کریں جب بنڈل میں نئے عرفات کو شامل کیا جائے تاکہ گار تازہ کاری کریں
وہی ثبوت۔

## مرحلہ 7 - DNS کٹ اوور ڈسریکٹر تیار کریں

پروڈکشن کٹ اوور کو آڈٹیبل ایکسچینج پیکٹ کی ضرورت ہوتی ہے۔ ایک کامیاب جمع کرانے کے بعد
(عرف بائنڈنگ) ، مددگار مسائل
`artifacts/sorafs/portal.dns-cutover.json` ، گرفتاری:

- عرف بائنڈنگ میٹا ڈیٹا (نام کی جگہ/نام/ثبوت ، منشور ڈائجسٹ ، یو آر ایل Torii ،
 بھیج دیا گیا عہد ، اتھارٹی) ؛
- ریلیز سیاق و سباق (ٹیگ ، عرف لیبل ، منشور/کار کے راستے ، چنک پلان ، بنڈل Sigstore) ؛
- توثیق کے اشارے (تحقیقات کمانڈ ، عرف + اختتامی نقطہ Torii) ؛ اور
- اختیاری تبدیلی پر قابو پانے والے فیلڈز (ٹکٹ ID ، کٹ اوور ونڈو ، OPS رابطہ ،
 میزبان نام/پروڈکشن زون) ؛
- روٹ پروموشن میٹا ڈیٹا ہیڈر `Sora-Route-Binding` سے ماخوذ ہے
 .
 گار اور فال بیک مشقیں ایک ہی ثبوت کا حوالہ دیتی ہیں۔
- تیار کردہ روٹ پلانٹ نمونے (`gateway.route_plan.json` ،
 ہیڈر ٹیمپلیٹس اور اختیاری رول بیک ہیڈر) تاکہ تبادلہ ٹکٹ اور تبادلہ ہکس
 CI لنٹ تصدیق کرسکتا ہے کہ ہر DG-3 پیکیج پروموشن/رول بیک منصوبوں کا حوالہ دیتا ہے
 منظوری سے پہلے کیننیکل دستاویزات ؛
- اختیاری کیشے باطل میٹا ڈیٹا (پرج اختتامی نقطہ ، آتھ متغیر ، JSON پے لوڈ ،
 اور کمانڈ مثال `curl`) ؛ اور
- رول بیک کے اشارے پچھلے ڈسکرپٹر کی طرف اشارہ کرتے ہیں (ریلیز ٹیگ اور مینی فیسٹ ڈائجسٹ)
 تاکہ ٹکٹوں نے ایک عین مطابق فال بیک بیک راستہ حاصل کیا۔

جب ریلیز میں کیشے کو صاف کرنے کی ضرورت ہوتی ہے تو ، یہ کٹ اوور ڈسکرپٹر کے ساتھ ایک کیننیکل پلان تیار کرتا ہے:

```bash
cargo xtask soradns-cache-plan \
 --name docs.sora \
 --path / \
 --path /gateway/manifest.json \
 --auth-header Authorization \
 --auth-env CACHE_PURGE_TOKEN \
 --json-out artifacts/sorafs/portal.cache_plan.json
```

نتیجے میں `portal.cache_plan.json` کو DG-3 پیکیج میں منسلک کریں تاکہ آپریٹرز
ٹینگن ڈٹرمینسٹک میزبان/راستے (اور اس سے ملنے والے اشارے) `PURGE` درخواستیں جاری کرنے کے لئے۔
اختیاری وضاحتی کیشے والا سیکشن اس فائل کا براہ راست حوالہ دے سکتا ہے ،
تبدیلی پر قابو پانے والے جائزہ لینے والوں کو بالکل ٹھیک رکھنا ہے کہ کس اختتامی نکات کو صاف کیا جارہا ہے
ایک کٹور کے دوران

ہر DG-3 پیکیج میں بھی پروموشن + رول بیک چیک لسٹ کی ضرورت ہوتی ہے۔ جنرلا کے ذریعے
`cargo xtask soradns-route-plan` تاکہ تبدیلی پر قابو پانے والے جائزہ لینے والے اقدامات پر عمل کرسکیں
عین مطابق پریف لائٹ ، کٹ اوور اور رول بیک الیاس:

```bash
cargo xtask soradns-route-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` نے صحت کی جانچ پڑتال کو یاد کیا ، کیننیکل/خوبصورت میزبانوں کی گرفتاری جاری کی
مراحل میں ، گار بائنڈنگ اپڈیٹس ، کیشے صاف اور رول بیک ایکشنز۔ اس کے ساتھ شامل کریں
ایکسچینج ٹکٹ بھیجنے سے پہلے گار/بائنڈنگ/کٹ اوور نمونے
اسکرپٹ کے ساتھ انہی اقدامات کو منظور کریں۔

`scripts/generate-dns-cutover-plan.mjs` اس ڈسریکٹر کو فروغ دیتا ہے اور اس کے بعد خود بخود چلتا ہے
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
```پن مددگار چلانے سے پہلے آس پاس کے متغیرات کے ذریعے اختیاری میٹا ڈیٹا کو ریلینس:

| متغیر | مقصد |
| --- | --- |
| `DNS_CHANGE_TICKET` | ڈسکرپٹر میں ٹکٹ کی شناخت محفوظ ہے۔ |
| `DNS_CUTOVER_WINDOW` | ISO8601 کٹ اوور ونڈو (جیسے `2026-03-21T15:00Z/2026-03-21T15:30Z`)۔ |
| `DNS_HOSTNAME` ، `DNS_ZONE` | پروڈکشن میزبان نام + مستند زون۔ |
| `DNS_OPS_CONTACT` | عرف آن کال یا بڑھتی ہوئی رابطہ۔ |
| `DNS_CACHE_PURGE_ENDPOINT` | ڈسکرپٹر میں رجسٹرڈ کیشے صاف کریں۔ |
| `DNS_CACHE_PURGE_AUTH_ENV` | env var جس میں پرج ٹوکن ہوتا ہے (پہلے سے طے شدہ: `CACHE_PURGE_TOKEN`)۔ |
| `DNS_PREVIOUS_PLAN` | رول بیک میٹا ڈیٹا کے لئے پچھلے کٹ اوور ڈسکرپٹر کا راستہ۔ |

JSON کو DNS تبدیلی کے جائزے سے منسلک کریں تاکہ منظوری دینے والے مینی فیسٹ ڈائجسٹ کو چیک کرسکیں ،
عرف بائنڈنگ اور تحقیقات کے احکامات CI لاگز کا جائزہ لینے کے بغیر۔ سی ایل آئی کے جھنڈے
`--dns-change-ticket` ، `--dns-cutover-window` ، `--dns-hostname` ،
`--dns-zone` ، `--ops-contact` ، `--cache-purge-endpoint` ،
`--cache-purge-auth-env` ، اور `--previous-dns-plan` وہی اوور رائڈس فراہم کرتا ہے
جب CI FUERA مددگار چلاتے ہو۔

## مرحلہ 8 - حل کرنے والے زون فائل کنکال (اختیاری)

جب پروڈکشن کٹ اوور ونڈو معلوم ہوجائے تو ، ریلیز اسکرپٹ جاری کرسکتا ہے
ایس این ایس زون فائل کنکال اور آٹو ریزول سنیپٹ۔ مطلوبہ DNS ریکارڈ پاس کریں
اور میٹا ڈیٹا آس پاس کے متغیرات یا سی ایل آئی کے اختیارات کے ذریعے۔ مددگار نے فون کیا
`scripts/sns_zonefile_skeleton.py` کٹ اوور ڈسکرپٹر تیار کرنے کے فورا بعد۔
کم از کم ایک A/AAAA/CNAME ویلیو اور GAR ڈائجسٹ (دستخط شدہ GAR پے لوڈ کا BLAK3-256) فراہم کریں۔ اگر
ہیلپر لکھتا ہے ، زونا/میزبان نام جانا جاتا ہے اور `--dns-zonefile-out` کو چھوڑ دیا گیا ہے
`artifacts/sns/zonefiles/<zone>/<hostname>.json` اور مکمل
`ops/soradns/static_zones.<hostname>.json` بطور حل ٹکڑا۔| متغیر / پرچم | مقصد |
| --- | --- |
| `DNS_ZONEFILE_OUT` ، `--dns-zonefile-out` | پیدا شدہ زون فائل کنکال کا راستہ۔ |
| `DNS_ZONEFILE_RESOLVER_SNIPPET` ، `--dns-zonefile-resolver-snippet` | اسنیپٹ روٹ کو حل کریں (پہلے سے طے شدہ: `ops/soradns/static_zones.<hostname>.json` جب چھوڑ دیا جاتا ہے)۔ |
| `DNS_ZONEFILE_TTL` ، `--dns-zonefile-ttl` | ٹی ٹی ایل نے تیار کردہ ریکارڈوں پر لاگو کیا (پہلے سے طے شدہ: 600 سیکنڈ)۔ |
| `DNS_ZONEFILE_IPV4` ، `--dns-zonefile-ipv4` | IPv4 سمت (کوما یا تکرار کرنے والے CLI پرچم کے ذریعہ الگ الگ)۔ |
| `DNS_ZONEFILE_IPV6` ، `--dns-zonefile-ipv6` | IPv6 سمت |
| `DNS_ZONEFILE_CNAME` ، `--dns-zonefile-cname` | اختیاری ہدف CNAME۔ |
| `DNS_ZONEFILE_SPKI` ، `--dns-zonefile-spki-pin` | پائینز ایس پی کے آئی شا -256 (بیس 64)۔ |
| `DNS_ZONEFILE_TXT` ، `--dns-zonefile-txt` | اضافی TXT اندراجات (`key=value`)۔ |
| `DNS_ZONEFILE_VERSION` ، `--dns-zonefile-version` | کمپیوٹڈ زون فائل ورژن لیبل کا اوور رائڈ۔ |
| `DNS_ZONEFILE_EFFECTIVE_AT` ، `--dns-zonefile-effective-at` | ٹائم اسٹیمپ `effective_at` (RFC3339) کو کٹ اوور ونڈو کے آغاز کے بجائے سیٹ کریں۔ |
| `DNS_ZONEFILE_PROOF` ، `--dns-zonefile-proof` | میٹا ڈیٹا میں ریکارڈ کردہ لفظی ثبوت کے اوور رائڈ۔ |
| `DNS_ZONEFILE_CID` ، `--dns-zonefile-cid` | میٹا ڈیٹا میں رجسٹرڈ سی آئی ڈی کا اوور رائڈ۔ |
| `DNS_ZONEFILE_FREEZE_STATE` ، `--dns-zonefile-freeze-state` | گارڈین منجمد ریاست (نرم ، سخت ، پگھلنا ، نگرانی ، ہنگامی صورتحال)۔ |
| `DNS_ZONEFILE_FREEZE_TICKET` ، `--dns-zonefile-freeze-ticket` | گارڈین/کونسل کے ٹکٹ کا حوالہ منجمد کرنے کے لئے۔ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT` ، `--dns-zonefile-freeze-expires-at` | پگھلنے کے لئے RFC3339 ٹائم اسٹیمپ۔ |
| `DNS_ZONEFILE_FREEZE_NOTES` ، `--dns-zonefile-freeze-note` | اضافی منجمد نوٹ (کوما اور تکرار کرنے والے پرچم کے ذریعہ الگ الگ)۔ |
| `DNS_GAR_DIGEST` ، `--dns-gar-digest` | دستخط شدہ GAR پے لوڈ کے بلیک 3-256 (ہیکس) کو ہضم کریں۔ جب گیٹ وے بائنڈنگز ہوں تو ضروری ہے۔ |

گٹ ہب ایکشن ورک فلو ان اقدار کو ذخیرے کے رازوں سے پڑھتا ہے تاکہ ہر پن
پیداوار زون فائل نمونے کو خود بخود خارج کرتی ہے۔ مندرجہ ذیل رازوں کو تشکیل دیں
(تاروں میں ملٹی ویلیو فیلڈز کے لئے کوما سے الگ الگ فہرستیں شامل ہوسکتی ہیں):

| خفیہ | مقصد |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME` ، `DOCS_SORAFS_DNS_ZONE` | میزبان نام/پروڈکشن زون ماضی کے مددگار۔ |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | آن کال عرفیئس ڈسکرپٹر میں محفوظ ہے۔ |
| `DOCS_SORAFS_ZONEFILE_IPV4` ، `DOCS_SORAFS_ZONEFILE_IPV6` | IPv4/IPv6 ریکارڈ شائع کیے جائیں گے۔ |
| `DOCS_SORAFS_ZONEFILE_CNAME` | اختیاری ہدف CNAME۔ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | پائنس ایس پی کے آئی بیس 64۔ |
| `DOCS_SORAFS_ZONEFILE_TXT` | اضافی TXT اندراجات۔ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | کنکال میں ریکارڈ شدہ میٹا ڈیٹا منجمد کریں۔ |
| `DOCS_SORAFS_GAR_DIGEST` | دستخط شدہ گار پے لوڈ کے ہیکس میں بلیک 3 کو ہضم کریں۔ |

`.github/workflows/docs-portal-sorafs-pin.yml` کو متحرک کرتے وقت ، یہ آدانوں کو فراہم کرتا ہے
`dns_change_ticket` اور `dns_cutover_window` تاکہ ڈسکرپٹر/زون فائل ونڈو سے وراثت میں مل جائے
درست. جب آپ رنز چلاتے ہو تب ہی انہیں خالی چھوڑ دیں۔

عام درخواست (مالک کی SN-7 رن بک کے ساتھ موافق ہے):

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

مددگار خود بخود تبادلے کے ٹکٹ کو TXT ان پٹ کے طور پر گھسیٹتا ہے اور اس کے آغاز کو وراثت میں ملتا ہے
جب تک اوور رائڈ نہیں ہوتا ہے تو ٹائم اسٹیمپ `effective_at` کے طور پر کٹ اوور ونڈو۔ بہاؤ کے لئے
مکمل آپریٹو ، `docs/source/sorafs_gateway_dns_owner_runbook.md` دیکھیں۔

### عوامی DNS وفد پر نوٹزون فائل کنکال صرف مستند زون کے ریکارڈوں کی وضاحت کرتا ہے۔ اب بھی اور
رجسٹرار یا فراہم کنندہ میں پیرنٹ زون کے NS/DS وفد کو تشکیل دینا ضروری ہے
DNS تاکہ انٹرنیٹ ناموں کو تلاش کرسکے۔

۔
  A/AAAA گیٹ وے کے کسی بھی کاسٹ IPs کی طرف اشارہ کرتے ہوئے ریکارڈ کرتا ہے۔
- ذیلی ڈومینز کے لئے ، اخذ کردہ خوبصورت میزبان کو ایک نام شائع کریں
  (`<fqdn>.gw.sora.name`)۔
- کیننیکل میزبان (`<hash>.gw.sora.id`) گیٹ وے ڈومین کے نیچے رہتا ہے اور نہیں ہے
  اور آپ کے عوامی زون میں شائع ہوا۔

### گیٹ وے ہیڈر لے آؤٹ

ڈیپلو مددگار `portal.gateway.headers.txt` اور بھی جاری کرتا ہے
`portal.gateway.binding.json` ، نمونے میں سے جو DG-3 کی ضرورت کو پورا کرتے ہیں
گیٹ وے-مشمولات کا پابند:

- `portal.gateway.headers.txt` میں HTTP ہیڈر (بشمول شامل ہے
 `Sora-Name` ، `Sora-Content-CID` ، `Sora-Proof` ، CSP ، HSTS ، اور تفصیل کار
 `Sora-Route-Binding`) اس ایج گیٹ ویز کو ہر جواب میں گرفت میں رکھنا چاہئے۔
- `portal.gateway.binding.json` مشین سے پڑھنے کے قابل شکل میں ایک ہی معلومات کو ریکارڈ کرتا ہے
 تاکہ تبادلے کے ٹکٹ اور آٹومیشن میزبان/سی آئی ڈی بائنڈنگ کے بغیر موازنہ کرسکیں
 شیل ٹپکنے کو ختم کریں۔

یہ خود بخود پیدا ہوتا ہے
`cargo xtask soradns-binding-template`
اور عرف پر قبضہ کریں ، ظاہر ڈائجسٹ ، اور گیٹوی کا میزبان نام جو ہے
`sorafs-pin-release.sh` میں پاس ہوا۔ ہیڈر بلاک کو دوبارہ تخلیق یا اپنی مرضی کے مطابق بنانا ،
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
جب کسی تعیناتی کو اضافی ہدایت کی ضرورت ہوتی ہے تو ہیڈر ٹیمپلیٹس کو پہلے سے طے کریں۔
ہیڈر کو مکمل طور پر ختم کرنے کے لئے ان کو موجودہ `--no-*` سوئچ کے ساتھ جوڑیں۔

سی ڈی این تبدیلی کی درخواست کے ساتھ ہیڈر کے ٹکڑوں کو منسلک کریں اور JSON دستاویز کو کھانا کھلائیں
AL گیٹ وے آٹومیشن پائپ لائن تاکہ اصل میزبان کو فروغ دینے سے میل مل سکے
ثبوت جاری کریں۔

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

کمانڈ جام شدہ `Sora-Proof` پے لوڈ کو ضابطہ کشائی کرتا ہے ، اس بات کو یقینی بناتا ہے کہ `Sora-Route-Binding` میٹا ڈیٹا
منشور + میزبان نام کے سی آئی ڈی سے میل کھاتا ہے ، اور اگر کوئی ہیڈر انحراف کرتا ہے تو یہ تیزی سے ناکام ہوجاتا ہے۔
جب بھی آپ چلتے ہیں دوسرے منقطع نمونے کے ساتھ کنسول آؤٹ پٹ فائل کریں
کمانڈ CI سے بھیجا گیا تھا تاکہ DG-3 جائزہ لینے والے یہ ثابت کرسکیں کہ پابند ہونے کی توثیق کی گئی تھی
کٹ اوور سے پہلے

> ** DNS ڈسکرپٹر انضمام: ** `portal.dns-cutover.json` اب ایک حصے کو سرایت کرتا ہے
> `gateway_binding` ان نمونے کی طرف اشارہ کرتے ہوئے (راستے ، سی آئی ڈی کا مواد ، ثبوت کی حیثیت اور
> لفظی ہیڈر ٹیمپلیٹ) ** اور ** ایک اسٹینزا `route_plan` حوالہ دینا
> `gateway.route_plan.json` لیکن اہم اور رول بیک ہیڈر ٹیمپلیٹس۔ ان کو شامل کریں
> ہر DG-3 ایکسچینج ٹکٹ پر بلاکس تاکہ جائزہ لینے والے اقدار کا موازنہ کرسکیں
> عین مطابق `Sora-Name/Sora-Proof/CSP` اور اس کی تصدیق کریں کہ اس پروموشن/رول بیک منصوبے
> بلڈ فائل کو کھولے بغیر ثبوت کے بنڈل سے میچ کریں۔## مرحلہ 9 - پبلشنگ مانیٹر چلائیں

روڈ میپ آئٹم ** دستاویزات -3 سی ** کے لئے جاری ثبوت کی ضرورت ہے کہ پورٹل ، اس کو پراکسی اور
رہائی کے بعد گیٹوی پابندیاں صحت مند رہیں۔ مستحکم مانیٹر چلائیں
7-8 قدموں کے فورا بعد اور اسے اپنے پروگرام شدہ تحقیقات سے مربوط کریں:

```bash
cd docs/portal
npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%E%m%dT%H%M%SZ).json \
 --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` کنفگ فائل کو لوڈ کریں (دیکھیں
 `docs/portal/docs/devportal/publishing-monitoring.md` اسکیمیٹک کے لئے) اور
 تین چیک چلائیں: پورٹل پاتھ پروبس + سی ایس پی/اجازت-پالیسی کی توثیق ،
 اس کو پراکسی تحقیقات (اختیاری طور پر اس کے `/metrics` اختتامی نقطہ کو مارنا) ، اور تصدیق کنندہ
 گیٹ وے بائنڈنگ (`cargo xtask soradns-verify-binding`) کی جس میں اب موجودگی کی ضرورت ہے اور
 عرف/مینی فیسٹ چیک کے ساتھ ساتھ سورہ-مشمولات سیڈ کی متوقع قیمت۔
- کمانڈ غیر صفر کے ساتھ ختم ہوجاتی ہے جب کوئی بھی تحقیقات ناکام ہوجاتی ہے تاکہ سی آئی ، کرون ملازمتیں ، یا آپریٹرز
 رن بک عرف کو فروغ دینے سے پہلے ایک ریلیز کا انعقاد کرسکتا ہے۔
- پاس `--json-out` ہر ہدف کی حیثیت کے ساتھ ایک JSON خلاصہ لکھیں۔ `--evidence-dir`
 `summary.json` ، `portal.json` ، `tryit.json` ، `binding.json` اور `checksums.sha256` تاکہ یہ جاری کریں
 گورننس کے جائزہ لینے والے مانیٹر کو دوبارہ چلائے بغیر نتائج کا موازنہ کرسکتے ہیں۔ آرکائیو
 یہ کم ڈائرکٹری `artifacts/sorafs/<tag>/monitoring/` بنڈل Sigstore اور کے ساتھ آگے
 DNS کٹ اوور ڈسریکٹر۔
- مانیٹر آؤٹ پٹ ، Grafana (`dashboards/grafana/docs_portal.json`) کی برآمد پر مشتمل ہے ،
 اور ریلیز ٹکٹ میں الرٹ مینجر ڈرل ID تاکہ DOCS-3C SLO ہوسکے
 آڈٹ لیوگو۔ سرشار پبلشنگ مانیٹر پلے بک میں رہتا ہے
 `docs/portal/docs/devportal/publishing-monitoring.md`۔

پورٹل تحقیقات کو HTTPS کی ضرورت ہوتی ہے اور بیس URLs کو مسترد کرنا `http://` جب تک `allowInsecureHttp`
یہ مانیٹر کنفیگ میں تشکیل دیا گیا ہے۔ پیداوار/اسٹیجنگ اہداف کو TLS اور صرف میں رکھیں
مقامی پیش نظاروں کے لئے اوور رائڈ کو قابل بناتا ہے۔

ایک بار پورٹل ایک بار بلڈکائٹ/کرون میں `npm run monitor:publishing` کے ذریعے مانیٹر کو خود کار کرتا ہے
یہ زندہ ہے۔ اسی کمانڈ ، جو پیداوار کے یو آر ایل کی طرف اشارہ کرتے ہیں ، صحت کی جانچ پڑتال کرتے ہیں
تسلسل جو SRE/دستاویزات ریلیز کے مابین استعمال کرتے ہیں۔

## `sorafs-pin-release.sh` کے ساتھ آٹومیشن

`docs/portal/scripts/sorafs-pin-release.sh` اقدامات 2-6 سے منسلک کرتا ہے۔ یہ:

1. آرکائیو `build/` ایک ڈٹرمینسٹک ٹربال میں ،
2. چلائیں `car pack` ، `manifest build` ، `manifest sign` ، `manifest verify-signature` ،
 اور `proof verify` ،
3. اختیاری طور پر `manifest submit` چلائیں (بشمول عرف بائنڈنگ) جب اسناد موجود ہوں
 Torii ، اور
4. `artifacts/sorafs/portal.pin.report.json` لکھیں ، اختیاری
 `portal.pin.proposal.json` ، DNS کٹ اوور ڈسکرپٹر (گذارشات کے بعد) ،
 اور گیٹ وے بائنڈنگ بنڈل (`portal.gateway.binding.json` لیکن ہیڈر بلاک)
 تاکہ گورننس ، نیٹ ورکنگ اور او پی ایس ٹیمیں ثبوت کے بنڈل کا موازنہ کرسکیں
 CI لاگز کا جائزہ لینے کے بغیر۔

`PIN_ALIAS` ، `PIN_ALIAS_NAMESPACE` ، `PIN_ALIAS_NAME` ، اور (اختیاری طور پر) تشکیل دیتا ہے
اسکرپٹ کی درخواست کرنے سے پہلے `PIN_ALIAS_PROOF_PATH`۔ خشک کے لئے `--skip-submit` استعمال کرتا ہے
رمز ؛ گٹ ہب ورک فلو ان پٹ `perform_submit` کے ذریعے ذیل میں بیان کیا گیا ہے۔

## مرحلہ 8 - OpenAPI نردجیکرن اور SBOM پیکیج شائع کریںDOCS-7 کا تقاضا ہے کہ پورٹل بلڈ ، OpenAPI تفصیلات اور SBOM نمونے کا سفر
اسی عزم پائپ لائن کے ذریعے۔ موجودہ مددگار تینوں کا احاطہ کرتے ہیں:

1. ** تصریح کو دوبارہ تخلیق کرتا ہے اور اس پر دستخط کرتا ہے۔ **

 ```bash
 npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
 cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
 ```

 جب آپ اسنیپ شاٹ کو محفوظ رکھنا چاہتے ہیں تو `--version=<label>` کے ذریعے ریلیز ٹیگ پاس کریں
 تاریخ (مثال کے طور پر `2025-q3`)۔ مددگار اسنیپ شاٹ لکھتا ہے
 `static/openapi/versions/<label>/torii.json` ، یہ انتظار کرتا ہے
 `versions/current` ، اور میٹا ڈیٹا کو ریکارڈ کرتا ہے (SHA-256 ، منشور ریاست ، اور
 `static/openapi/versions.json` میں تازہ ترین ٹائم اسٹیمپ)۔ ڈویلپر پورٹل
 اس انڈیکس کو پڑھیں تاکہ سویگر/ریپڈوک پینل ایک ورژن سلیکٹر کو ظاہر کرسکیں
 اور لائن میں وابستہ ڈائجسٹ/دستخط دکھائیں۔ `--version` کو چھوڑ دینا `--version` لیبل رکھتا ہے
 پچھلی ریلیز برقرار ہے اور صرف `current` + `latest` پوائنٹس کو تازہ دم کرتی ہے۔

 مینی فیسٹ نے SHA-256/BLAKE3 ڈائجسٹز کو اپنی گرفت میں لے لیا تاکہ گیٹ وے لپیٹ سکے
 ہیڈر `Sora-Proof` سے `/reference/torii-swagger`۔

2.
 `docs/source/sorafs_release_pipeline_plan.md` کے مطابق۔ آؤٹ پٹ رکھیں
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

 مرکزی سائٹ کے طور پر اسی `manifest build` / `manifest sign` اقدامات پر عمل کریں ،
 ہر نمونے میں عرف کی ترتیب (جیسے `docs-openapi.sora` تفصیلات کے لئے اور
 دستخط شدہ SBOM بنڈل کیلئے `docs-sbom.sora`)۔ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ الگ
 گارس اور رول بیک ٹکٹ عین مطابق پے لوڈ کے لئے مختص ہیں۔

4. ** جمع کروائیں اور پابند ہوں۔
 آڈیٹرز کے لئے سورا کے ناموں کو ٹریک کرنے کے لئے ریلیز چیک لسٹ میں عرف ٹوپل
 ہاضمے کو ظاہر کرنے کے نقشے۔

پورٹل بلڈ کے ساتھ قیاس آرائی/ایس بی او ایم کا پتہ لگانا یقینی بناتا ہے کہ ہر ٹکٹ
ریلیز میں پیکر کو دوبارہ چلائے بغیر نمونے کا مکمل سیٹ شامل ہے۔

### آٹومیشن ہیلپر (CI/پیکیج اسکرپٹ)

`./ci/package_docs_portal_sorafs.sh` 1-8 کے قدموں کو انکوڈ کرتا ہے تاکہ روڈ میپ آئٹم
** DOCS-7 ** صرف ایک کمانڈ کے ساتھ استعمال کیا جاسکتا ہے۔ مددگار:

- پورٹل (`npm ci` ، Sync OpenAPI/نوریٹو ، ویجیٹ ٹیسٹ) کی مطلوبہ تیاری انجام دیں۔
- `sorafs_cli` کے ذریعے کاریں اور پورٹل منشور جوڑے ، OpenAPI اور SBOM جاری کرتے ہیں۔
- اختیاری طور پر `sorafs_cli proof verify` (`--proof`) اور دستخط Sigstore پر عمل کریں
 (`--sign` ، `--sigstore-provider` ، `--sigstore-audience`) ؛
- ڈیجا تمام نوادرات کم `artifacts/devportal/sorafs/<timestamp>/` اور
 `package_summary.json` لکھیں تاکہ CI/ریلیز ٹولنگ بنڈل کو کھا سکے۔ اور
- حالیہ عملدرآمد کی طرف اشارہ کرنے کے لئے `artifacts/devportal/sorafs/latest` کو تازہ کرتا ہے۔

مثال (Sigstore + POR کے ساتھ مکمل پائپ لائن):

```bash
./ci/package_docs_portal_sorafs.sh \
 --proof \
 --sign \
 --sigstore-provider=github-actions \
 --sigstore-audience=sorafs-devportal
```

جاننے کے لئے جھنڈے:- `--out <dir>` - نمونے کی جڑ اوور رائڈ (ڈیفالٹ ٹائم اسٹیمپ کے ساتھ قالینوں کو رکھتا ہے)۔
- `--skip-build` - موجودہ `docs/portal/build` (جب CI نہیں کرسکتا ہے تو مفید ہے
 آف لائن آئینے کے ذریعہ دوبارہ تعمیر)۔
- `--skip-sync-openapi` - `npm run sync-openapi` کو چھوڑ دیتا ہے جب `cargo xtask openapi`
 کریٹس تک نہیں پہنچ سکتا۔
- `--skip-sbom` - جب بائنری انسٹال نہیں ہوتا ہے تو `syft` کو فون کرنے سے گریز کرتا ہے (اسکرپٹ پرنٹس
 اس کی جگہ پر ایک انتباہ)۔
- `--proof` - ہر کار/منشور جوڑی کے لئے `sorafs_cli proof verify` چلائیں۔ کے پے لوڈ
 متعدد فائلوں کو CLI پر CHUNK-PLAN کی مدد کی ضرورت نہیں ہے ، لہذا یہ جھنڈا طے کیا گیا ہے
 اگر آپ کو `plan chunk count` سے غلطیوں کا سامنا کرنا پڑتا ہے اور جب دستی طور پر چیک کریں
 upstream گیٹ تک پہنچیں۔
- `--sign` - `sorafs_cli manifest sign` کی درخواست کرتا ہے۔ کے ساتھ ایک ٹوکن ثابت کریں
 `SIGSTORE_ID_TOKEN` (یا `--sigstore-token-env`) یا CLI اس کا استعمال کرتے ہوئے اسے حاصل کرتا ہے
 `--sigstore-provider/--sigstore-audience`۔

پروڈکشن نمونے بھیجتے وقت ، `docs/portal/scripts/sorafs-pin-release.sh` استعمال کریں۔
اب پورٹل ، OpenAPI اور SBOM پے لوڈز پیکجز ، ہر مینی فیسٹ پر دستخط کرتا ہے ، اور میٹا ڈیٹا ریکارڈ کرتا ہے
`portal.additional_assets.json` میں اضافی اثاثے۔ مددگار ایک ہی اختیاری نوبس کو سمجھتا ہے
CI پیکیجر کے ذریعہ استعمال کیا جاتا ہے لیکن نیا سوئچ `--openapi-*` ، `--portal-sbom-*` ، اور
`--openapi-sbom-*` فی نمونہ پر عرف ٹوپل تفویض کرنے کے لئے ، SBOM ماخذ کے ذریعہ اوور رائڈ
`--openapi-sbom-source` ، کچھ پے لوڈ (`--skip-openapi`/`--skip-sbom`) کو چھوڑ دیں ،
اور `--syft-bin` کے ساتھ بطور ڈیفالٹ بائنری `syft` کی طرف اشارہ کریں۔

اسکرپٹ ہر کمانڈ کو بے نقاب کرتا ہے جس پر عمل درآمد ہوتا ہے۔ لاگ کو ریلیز ٹکٹ میں کاپی کریں
`package_summary.json` کے آگے تاکہ جائزہ لینے والے کار ڈائجسٹ کا موازنہ کرسکیں ،
پلان ، اور بنڈل ہیش Sigstore ایڈہاک شیل آؤٹ پٹ کا جائزہ لینے کے بغیر۔

## مرحلہ 9 - گیٹ وے کی توثیق + soradns

کٹ اوور کا اعلان کرنے سے پہلے ، یہ ثابت کریں کہ نیا عرف سوردنز کے ذریعہ دوبارہ زندہ ہے اور گیٹ وے
اینگراپن کے تازہ ثبوت:

1. ** تحقیقات کے گیٹ کو چلائیں۔ ** `ci/check_sorafs_gateway_probe.sh` Ejercita
 `cargo xtask sorafs-gateway-probe` میں ڈیمو فکسچر کے خلاف
 `fixtures/sorafs_gateway/probe_demo/`۔ حقیقی تعیناتیوں کے لئے ، ہدف کے میزبان نام پر تحقیقات کی نشاندہی کریں:

 ```bash
 ./ci/check_sorafs_gateway_probe.sh -- \
 --gatewae "https://docs.sora/.well-known/sorafs/manifest" \
 --header "Accept: application/json" \
 --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
 --gar-kee "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
 --host "docs.sora" \
 --report-json artifacts/sorafs_gateway_probe/ci/docs.json
 ```

 تحقیقات `Sora-Name` ، `Sora-Proof` ، اور `Sora-Proof-Status` کے مطابق
 `docs/source/sorafs_alias_policy.md` اور مینی فیسٹ کو ہضم کرتے وقت ناکام ہوجاتا ہے ،
 ٹی ٹی ایل اور گار پابندیاں انحراف کرتی ہیں۔

   ہلکا پھلکا اسپاٹ چیک کے لئے (مثال کے طور پر ، جب صرف پابند بنڈل
   تبدیل شدہ) ، `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>` چلائیں۔
   مددگار پکڑے گئے بائنڈنگ بنڈل کی توثیق کرتا ہے اور رہائی کے لئے آسان ہے
   وہ ٹکٹ جن کو صرف مکمل تحقیقات کی مشق کی بجائے پابند تصدیق کی ضرورت ہوتی ہے۔

2. ** ڈرل شواہد پر قبضہ کرتا ہے۔
 `اسکرپٹ/ٹیلی میٹری/رن_سورافس_گیٹ وے_پروبی.ش -سکینریو کے ساتھ تحقیقات
 devuporal-rollout-... `. ریپر کم ہیڈر/لاگز کو بچاتا ہے
 `artifacts/sorafs_gateway_probe/<stamp>/` ، تازہ ترین `ops/drill-log.md` ، اور
 (اختیاری طور پر) رول بیک ہکس یا پیجریڈی پے لوڈ کو متحرک کرتا ہے۔ قائم کریں
 `--host docs.sora` IP کو سخت کوڈنگ کے بجائے سورادنس روٹ کی توثیق کرنے کے لئے۔3. ** DNS پابندیاں چیک کریں۔ ** جب حکومت عرف کا ثبوت شائع کرتی ہے تو ، یہ GAR فائل کو رجسٹر کرتی ہے
 تحقیقات (`--gar`) کے ذریعہ حوالہ دیا گیا ہے اور رہائی کے ثبوت سے منسلک ہے۔ حل کرنے کے لئے مالکان
 `tools/soradns-resolver` کے ذریعہ ایک ہی ان پٹ کی عکاسی کرسکتا ہے تاکہ اس بات کو یقینی بنایا جاسکے کہ کیشڈ اندراجات
 نئے منشور کا احترام کریں۔ JSON منسلک کرنے سے پہلے ، چلائیں
 `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
 تاکہ ڈٹرمینسٹک میزبان میپنگ ، مینی فیسٹ میٹا ڈیٹا ، اور ٹیلی میٹر ٹیگز میچ ہوں
 درست آف لائن۔ مددگار دستخط شدہ گار کے ساتھ مل کر ایک سمری `--json-out` جاری کرسکتا ہے تاکہ
 ٹینگن جائزہ لینے والے بائنری کھولے بغیر تصدیق کے ثبوت۔
 نیا گار لکھتے وقت ، ترجیح دیں
 `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
 (`--manifest-cid <cid>` پر واپس جائیں جب صرف مینی فیسٹ فائل دستیاب نہ ہو)۔ مددگار
 اب سی آئی ڈی ** اور ** بلیک 3 ڈائجسٹ کو براہ راست JSON منشور سے اخذ کرتا ہے ، خالی جگہوں کو کاٹ دیتا ہے ،
 دہرائے ہوئے `--telemetry-label` جھنڈوں کو دہراتا ہے ، لیبلوں کا آرڈر دیتا ہے ، اور پہلے سے طے شدہ ٹیمپلیٹس کو آؤٹ پٹ کرتا ہے
 JSON لکھنے سے پہلے CSP/HSTS/اجازت-پولس کی تاکہ پے لوڈ کا بوجھ باقی رہ جائے
 یہاں تک کہ جب آپریٹرز مختلف گولوں سے ٹیگ حاصل کرتے ہیں۔

4. ** عرفی میٹرکس کا مشاہدہ کریں۔ ** `torii_sorafs_alias_cache_refresh_duration_ms` کو برقرار رکھیں
 اور `torii_sorafs_gateway_refusals_total{profile="docs"}` ڈسپلے پر
 چلائیں ؛ دونوں سیریز `dashboards/grafana/docs_portal.json` میں لکھی گئی ہیں۔

## مرحلہ 10 - نگرانی اور ثبوت کا بنڈل

- ** ڈیش بورڈز۔
 `dashboards/grafana/sorafs_gateway_observability.json` (گیٹ وے لیٹینسی +
 پروف ہیلتھ) ، اور `dashboards/grafana/sorafs_fetch_observability.json`
 (آرکسٹریٹر سالود) ہر ریلیز کے لئے۔ JSON برآمدات کو ٹکٹ پر منسلک کریں
 جاری کریں تاکہ جائزہ لینے والے Prometheus سوالات کو دوبارہ پیش کرسکیں۔
- ** تحقیقات فائلیں
 آپ کے ثبوت بالٹی۔ تحقیقات کا خلاصہ ، ہیڈر ، اور پیجریڈی پے لوڈ شامل کریں
 ٹیلی میٹری اسکرپٹ کے ذریعہ پکڑا گیا۔
- ** ریلیز بنڈل۔
 دستخط Sigstore ، `portal.pin.report.json` ، TRY-IT تحقیقات لاگز ، اور لنک چیک رپورٹس
 ٹائم اسٹیمپ والے قالین کے تحت (مثال کے طور پر ، `artifacts/sorafs/devportal/20260212T1103Z/`)۔
- ** ڈرل لاگ۔ ** جب تحقیقات کسی ڈرل کا حصہ ہوں تو انہیں اجازت دیں
 `scripts/telemetry/run_sorafs_gateway_probe.sh` `ops/drill-log.md` میں اندراجات شامل کریں
 تاکہ وہی ثبوت SNNET-5 افراتفری کی ضرورت کو پورا کریں۔
- ** ٹکٹ کے لنکس۔
 تحقیقات کی اطلاع دہندگی کے راستے کے ساتھ ، تبادلہ کی شرح ، تاکہ جائزہ لینے والے ایس ایل او کو عبور کرسکیں
 کوئی شیل تک رسائی نہیں ہے۔

## مرحلہ 11 - ملٹی سورس بازیافت ڈرل اور اسکور بورڈ ثبوت

SoraFS میں شائع کریں اب ملٹی سورس بازیافت ثبوت (DOCS-7/SF-6) کی ضرورت ہے۔
پچھلے ڈی این ایس/گیٹ وے شواہد کے ساتھ۔ منشور ترتیب دینے کے بعد:1. ** براہ راست منشور کے خلاف `sorafs_fetch` چلائیں۔ ** ایک ہی منصوبہ/ظاہر نمونے استعمال کرتا ہے
 اقدامات 2-3 میں تیار کیا گیا لیکن گیٹ وے کی اسناد ہر فراہم کنندہ کو جاری کی گئیں۔ برقرار رہتا ہے
 تمام آؤٹ پٹ تاکہ آڈیٹر آرکسٹریٹر کے فیصلے کی پگڈنڈی کو دوبارہ پیش کرسکیں:

 ```bash
 OUT=artifacts/sorafs/devportal
 FETCH_OUT="$OUT/fetch/$(date -u +%E%m%dT%H%M%SZ)"
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

 - منشور کے ذریعہ پیش کردہ فراہم کنندگان کے اشتہارات کی پہلی تلاش کریں (مثال کے طور پر
 `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
 اور ان کو `--provider-advert name=path` کے ذریعے پاس کریں تاکہ اسکور بورڈ ونڈوز کا اندازہ کرسکے
 ایک تعصب پسندانہ انداز میں صلاحیت کا۔ استعمال کریں
 `--allow-implicit-provider-metadata` ** صرف ** جب CI میں فکسچر کو دوبارہ تیار کیا جاتا ہے۔ مشقیں
 پروڈکشن ٹیم کو لازمی طور پر دستخط شدہ اشتہارات کا حوالہ دینا ہوگا جو پن کے ساتھ پہنچے۔
 - جب ظاہر ہوتا ہے اضافی خطوں کا حوالہ دیتا ہے تو ، کمانڈ کو ٹیوپلس کے ساتھ دہرائیں
 متعلقہ فراہم کنندگان تاکہ ہر کیشے/عرف سے وابستہ بازیافت کا نمونہ ہو۔

2. ** آؤٹ پٹس کو محفوظ کریں۔
 `providers.ndjson` ، `fetch.json` ، اور `chunk_receipts.ndjson` ثبوت قالین کے تحت
 ریلیز یہ فائلیں ہم مرتبہ وزن ، ریٹریچ بجٹ ، EWMA لیٹینسی ، اور پر قبضہ کرتی ہیں
 فی حصہ رسیدیں کہ گورننس پیکیج کو SF-7 کے لئے برقرار رکھنا چاہئے۔

3. ** تازہ ترین ٹیلیفون۔
 مشاہدہ ** (`dashboards/grafana/sorafs_fetch_observability.json`) ،
 `torii_sorafs_fetch_duration_ms`/`_failures_total` اور فراہم کنندہ کے ذریعہ رینج پینلز کا مشاہدہ کرنا
 بے ضابطگیوں کے لئے۔ Grafana پینل اسنیپ شاٹس کو ریلیز ٹکٹ کے ساتھ ساتھ اس کے راستے کے ساتھ منسلک کریں
 اسکور بورڈ.

4. ** انتباہ کے قواعد تمباکو نوشی کریں۔ ** `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں
 رہائی بند کرنے سے پہلے Prometheus الرٹ بنڈل کو توثیق کرنے کے لئے۔ آؤٹ پٹ منسلک کریں
 اس بات کی تصدیق کے ل doc دستاویزات -7 جائزہ لینے والوں کے لئے پرومٹول ال ٹکٹ اور اس اسٹال الرٹس اور
 سست فراہم کرنے والا مسلح رہتا ہے۔

5. ** CI میں کیبل۔ ** پورٹل پن ورک فلو ان پٹ کے پیچھے ایک قدم `sorafs_fetch` برقرار رکھتا ہے
 `perform_fetch_probe` ؛ اس کو اسٹیجنگ/پروڈکشن پھانسی کے ل enable قابل بنائیں تاکہ اس کا ثبوت ہو
 بازیافت دستی مداخلت کے بغیر مینی فیسٹ بنڈل کے ساتھ ساتھ تیار کی جاتی ہے۔ مقامی مشقیں کر سکتی ہیں
 گیٹ وے ٹوکن برآمد کرکے اور `PIN_FETCH_PROVIDERS` قائم کرکے اسی اسکرپٹ کو دوبارہ استعمال کریں
 کوما کے ذریعہ الگ کردہ فراہم کنندگان کی فہرست۔

## تشہیر ، مشاہدہ اور رول بیک

1. ** پروموشن: ** اسٹیجنگ اور پروڈکشن کے لئے الگ الگ عرفیت رکھیں۔ دوبارہ چلنے کو فروغ دیتا ہے
 `manifest submit` اسی مینی فیسٹ/بنڈل کے ساتھ ، بدل رہا ہے
 `--alias-namespace/--alias-name` پروڈکشن عرف میں داخل ہونے کے لئے۔ یہ گریز کرتا ہے
 ایک بار جب QA اسٹیجنگ پن کی تصدیق کرتا ہے تو دوبارہ تعمیر یا دوبارہ دستخط کریں۔
2. ** نگرانی: ** پن رجسٹری ڈیش بورڈ درآمد کریں
 (`docs/source/grafana_sorafs_pin_registry.json`) لیکن پورٹل سے متعلق تحقیقات
 (`docs/portal/docs/devportal/observability.md` دیکھیں)۔ چیکسم ڈرفٹ الرٹ ،
 ناکام تحقیقات یا ٹیسٹ ریٹریچ اسپائکس۔
3. ** رول بیک: ** رول بیک کے لئے ، پچھلے مینی فیسٹ (یا موجودہ عرف کو ہٹا دیں) کا استعمال کرتے ہوئے دوبارہ بھیج دیں
 `sorafs_cli manifest submit --alias ... --retire`۔ ہمیشہ تازہ ترین بنڈل رکھیں
 گڈ اور کار سمری کے نام سے جانا جاتا ہے تاکہ رول بیک ٹیسٹ کو دوبارہ بنایا جاسکے اگر
 CI لاگز گھومتے ہیں۔

## CI ورک فلو اسپریڈشیٹکم از کم کے طور پر ، آپ کی پائپ لائن لازمی ہے:

1. بلڈ + لنٹ (`npm ci` ، `npm run build` ، چیکسم جنریشن)۔
2. پیکیج (`car pack`) اور کمپیوٹ مینی فیسٹ۔
3. نوکری ٹوکن OIDC (`manifest sign`) کا استعمال کرتے ہوئے دستخط کریں۔
4. آڈٹ کے لئے نمونے (کار ، منشور ، بنڈل ، منصوبہ ، خلاصہ) اپ لوڈ کریں۔
5. ال پن رجسٹری بھیجیں:
 - درخواستیں کھینچیں -> `docs-preview.sora`۔
 - محفوظ ٹیگز/شاخیں -> پیداوار عرف پروموشن۔
6. تکمیل سے پہلے تحقیقات + پروف تصدیق کے دروازے چلائیں۔

`.github/workflows/docs-portal-sorafs-pin.yml` دستی ریلیز کے ل these ان تمام مراحل کو جوڑتا ہے۔
ورک فلو:

- پورٹل کی تعمیر/جانچ ،
- `scripts/sorafs-pin-release.sh` کے ذریعے تعمیر کے پیکیجز ،
- GITHUB OIDC کا استعمال کرتے ہوئے منشور بنڈل کی تصدیق/تصدیق کریں ،
- اپ لوڈ کار/منشور/بنڈل/منصوبہ/خلاصے بطور نمونے ، اور
- (اختیاری طور پر) جب خفیہ راز ہے تو مینی فیسٹ + عرف بائنڈنگ بھیجیں۔

نوکری شروع کرنے سے پہلے درج ذیل راز/متغیرات کے ذخیرے کی تشکیل کریں:

| نام | مقصد |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | میزبان Torii جو `/v1/sorafs/pin/register` کو بے نقاب کرتا ہے۔ |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | گذارشات کے ساتھ رجسٹرڈ ایپچ شناخت کنندہ۔ |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | ظاہر جمع کروانے کے لئے دستخطی اتھارٹی۔ |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | جب `perform_submit` `true` ہے تو عرف ٹپل نے منشور سے منسلک کیا ہے۔ |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | بیس 64 انکوڈڈ عرف پروف بنڈل (اختیاری ؛ عرف بائنڈنگ کو چھوڑنے کے لئے چھوڑ دیں)۔ |
| `DOCS_ANALYTICS_*` | موجودہ تجزیات/تحقیقات کے اختتامی مقامات دوسرے ورک فلوز کے ذریعہ دوبارہ استعمال کیے جاتے ہیں۔ |

کاموں کے ذریعے ورک فلو کو متحرک کریں UI:

1. پروئ `alias_label` (جیسے `docs.sora.link`) ، اختیاری `proposal_alias` ،
 اور `release_tag` کا ایک اختیاری اوور رائڈ۔
2. ڈیجا `perform_submit` بغیر Torii کو چھوئے بغیر نمونے تیار کرنے کی جانچ کیے بغیر
 (DRE رنز کے لئے مفید ہے) آپ کو براہ راست تشکیل شدہ عرف میں شائع کرنے کے قابل بناتا ہے۔

`docs/source/sorafs_ci_templates.md` عام سی آئی مددگاروں کے لئے بھی دستاویز کرتا ہے
پروجیکٹس اس ریپو سے آئے تھے ، لیکن پورٹل ورک فلو ترجیحی آپشن ہونا چاہئے
روزمرہ کی ریلیز کے لئے۔

## چیک لسٹ

- [] `npm run build` ، `npm run test:*` ، اور `npm run check:links` سبز رنگ میں ہیں۔
- [] `build/checksums.sha256` اور `build/release.json` نمونے میں پکڑے گئے۔
- [] کار ، منصوبہ ، منشور ، اور خلاصہ `artifacts/` کے تحت تیار کیا گیا ہے۔
- [] بنڈل Sigstore + لاگوں کے ساتھ محفوظ علیحدہ دستخط۔
- [] `portal.manifest.submit.summary.json` اور `portal.manifest.submit.response.json`
 جب گذارشات ہوں تو پکڑا گیا۔
- [] `portal.pin.report.json` (اور اختیاری `portal.pin.proposal.json`)
 کار/منشور کے نمونے کے ساتھ محفوظ شدہ۔
- [] `proof verify` اور `manifest verify-signature` آرکائویٹڈ کے نوشتہ جات۔
- [] تازہ ترین Grafana ڈیش بورڈز + کامیاب TRY-IT تحقیقات۔
- [] رول بیک نوٹ (پچھلے مینی فیسٹ ID + عرف ڈائجسٹ) میں شامل
 جاری ٹکٹ۔