---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# واقعہ رن بکس اور رول بیک مشقیں

## مقصد

روڈ میپ آئٹم ** دستاویزات -9 ** کو قابل عمل پلے بوکس کے علاوہ ایک پریکٹس پلان کی ضرورت ہے تاکہ
پورٹل آپریٹرز بغیر کسی اندازہ کے ترسیل کی ناکامیوں سے صحت یاب ہوسکتے ہیں۔ یہ نوٹ
تین اعلی سگنل واقعات کا احاطہ کرتا ہے: ناکام تعیناتی ، نقل کی انحطاط ، اور
تجزیات ڈراپ کرتے ہیں ، اور سہ ماہی مشقوں کی دستاویز کرتے ہیں جو یہ ثابت کرتے ہیں کہ رول بیک بیک ہے
عرفی اور مصنوعی توثیق ابھی بھی ختم ہونے کے لئے کام کرتی ہے۔

### متعلقہ مواد

- [`devportal/deploy-guide`] (./deploy-guide) - پیکیجنگ ، دستخط اور عرف پروموشن فلو۔
-.
- `docs/source/sorafs_node_client_protocol.md`
  اور [`sorafs/pin-registry-ops`] (../sorafs/pin-registry-ops)
  - رجسٹری ٹیلی میٹری اور اسکیلیشن دہلیز۔
- `docs/portal/scripts/sorafs-pin-release.sh` اور مددگار `npm run probe:*`
  چیک لسٹس میں حوالہ دیا گیا۔

### مشترکہ ٹیلی میٹری اور ٹولنگ

| سگنل / ٹول | مقصد |
| --------------- | ------- |
| `torii_sorafs_replication_sla_total` (میٹ/مس/زیر التواء) | نقل کے بلاکس اور ایس ایل اے کے فرق کا پتہ لگاتا ہے۔ |
| `torii_sorafs_replication_backlog_total` ، `torii_sorafs_replication_completion_latency_epochs` | ٹریج کے لئے بیک بلاگ کی گہرائی اور تکمیل میں تاخیر کی مقدار۔ |
| `torii_sorafs_gateway_refusals_total` ، `torii_sorafs_manifest_submit_total{status="error"}` | گیٹ وے کی ناکامیوں کو ظاہر کرتا ہے جو اکثر ناکام تعیناتی کی پیروی کرتے ہیں۔ |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | مصنوعی تحقیقات جو کرال رول بیکس کو جاری کرتی ہیں اور اس کی توثیق کرتی ہیں۔ |
| `npm run check:links` | ٹوٹا ہوا لنک گیٹ ؛ یہ ہر تخفیف کے بعد استعمال ہوتا ہے۔ |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` کے ذریعہ استعمال کیا جاتا ہے) | عرف پروموشن/ریجن میکانزم۔ |
| `Docs Portal Publishing` Grafana بورڈ (`dashboards/grafana/docs_portal.json`) | انکار/عرفی/TLS/نقل ٹیلی میٹری شامل کرتا ہے۔ پیجریڈی الرٹس ان پینلز کو بطور ثبوت حوالہ دیتے ہیں۔ |

## رن بک - ناکام تعیناتی یا غلط نمونے

### ٹرگر شرائط

- پیش نظارہ/پیداوار کی تحقیقات ناکام (`npm run probe:portal -- --expect-release=...`)۔
- `torii_sorafs_gateway_refusals_total` پر Grafana کو انتباہات یا
  `torii_sorafs_manifest_submit_total{status="error"}` ایک رول آؤٹ کے بعد۔
- دستی QA ٹوٹے ہوئے راستوں یا پراکسی کی ناکامیوں کا پتہ لگاتا ہے اس کے فورا بعد ہی اسے آزمائیں
  عرف پروموشن۔

### فوری طور پر کنٹینمنٹ

1. ** منجمد تعیناتی: ** CI پائپ لائن کو `DEPLOY_FREEZE=1` کے ساتھ نشان زد کریں (ان پٹ
   گٹ ہب) یا جینکنز کی نوکری کو روکیں تاکہ مزید نمونے ظاہر نہ ہوں۔
2.
   `portal.manifest*.{json,to,bundle,sig}` ، اور ناکام تعمیر کی تحقیقات کی پیداوار تاکہ
   رول بیک ٹھیک ہضموں کا حوالہ دیتا ہے۔
3. ** اسٹیک ہولڈرز کو مطلع کریں: ** اسٹوریج ایس آر ای ، دستاویزات/ڈیوریل لیڈ ، اور ڈیوٹی آفیسر
   بیداری کے لئے گورننس (خاص طور پر جب `docs.sora` متاثر ہوتا ہے)۔

### رول بیک طریقہ کار

1. آخری معلوم اچھ (ا (ایل کے جی) ظاہر کی شناخت کریں۔ پروڈکشن ورک فلو ان کو بچاتا ہے
   `artifacts/devportal/<release>/sorafs/portal.manifest.to` کے تحت۔
2. عرف کو دوبارہ جمع کرانے والے کے ساتھ اس عرفی کو دوبارہ لنک کریں:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. واقعے کے ٹکٹ پر رول بیک کا خلاصہ ریکارڈ کریں
   منشور ایل کے جی اور ناکام مینی فیسٹ۔

### توثیق1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` اور `sorafs_cli proof verify ...`
   (تعیناتی گائیڈ ملاحظہ کریں) اس بات کی تصدیق کرنے کے لئے کہ دوبارہ ترقی یافتہ مینی فیسٹ ابھی باقی ہے
   آرکائیو کار کے ساتھ ہم آہنگ۔
4. `npm run probe:tryit-proxy` اس بات کو یقینی بنانے کے لئے کہ کوشش کی جانے والی پراکسی واپس آجائے۔

### بعد کے بعد

1. بنیادی وجہ کو سمجھنے کے بعد ہی تعیناتی پائپ لائن کی بحالی کریں۔
2. [`devportal/deploy-guide`] (./deploy-guide) میں "سیکھے گئے" اندراجات کو پُر کریں
   نئے نوٹ کے ساتھ ، اگر قابل اطلاق ہو۔
3. ناکام ٹیسٹ سویٹ (تحقیقات ، لنک چیکر ، وغیرہ) کے لئے کھلے نقائص۔

## رن بک - نقل کو کم کرنا

### ٹرگر شرائط

- الرٹ: `رقم (torii_sorafs_replication_sla_total {نتیجہ =" میٹ "}) /
  کلیمپ_مین (رقم (torii_sorafs_replication_sla_total {نتیجہ = ~ "met | med | med"}) ، 1)  15 منٹ کے لئے۔
- رازداری کا جائزہ ضائع ہونے والے واقعات میں غیر متوقع اضافہ کا پتہ لگاتا ہے۔
- `npm run probe:tryit-proxy` راستوں پر ناکام ہوتا ہے `/probe/analytics`۔

### جواب1. بلڈ ان پٹ چیک کریں: `DOCS_ANALYTICS_ENDPOINT` اور
   `DOCS_ANALYTICS_SAMPLE_RATE` ریلیز آرٹیکٹیکٹ (`build/release.json`) میں۔
2. `DOCS_ANALYTICS_ENDPOINT` کی طرف اشارہ کرنے کے ساتھ `npm run probe:portal` کو دوبارہ ایگزیکٹو کریں
   اس بات کی تصدیق کرنے کے لئے کلیکٹر کا اسٹیج کرنا کہ ٹریکر ابھی بھی پے لوڈ کا اخراج کررہا ہے۔
3. اگر جمع کرنے والے نیچے ہیں تو ، `DOCS_ANALYTICS_ENDPOINT=""` سیٹ کریں اور دوبارہ تعمیر کریں
   ٹریکر کو شارٹ سرکٹ کے لئے ؛ آؤٹ پٹ ونڈو میں رجسٹر کرتا ہے
   واقعے کی ٹائم لائن۔
4. توثیق کریں کہ `scripts/check-links.mjs` فنگر پرنٹ `checksums.sha256` جاری رکھے ہوئے ہے
   (تجزیات کے قطرے * کو * سائٹ کا نقشہ کی توثیق کو روکنا نہیں چاہئے)۔
5. جب جمع کرنے والا صحت یاب ہوتا ہے تو ، عملدرآمد کے لئے `npm run test:widgets` چلائیں
   دوبارہ شائع ہونے سے پہلے تجزیاتی مددگار کے یونٹ ٹیسٹ۔

### بعد کے بعد

1. اپ ڈیٹ [`devportal/observability`] (./observability) کی نئی حدود کے ساتھ
   کلکٹر یا نمونے لینے کی ضروریات۔
2۔ گورننس کا نوٹس جاری کریں اگر تجزیات کا ڈیٹا کھو گیا یا باہر کو دوبارہ تیار کیا گیا ہو
   سیاست کی

## سہ ماہی لچکدار مشقیں

ہر سہ ماہی کے ** پہلے منگل کے دوران دونوں مشقیں چلائیں ** (جنوری/اپریل/جولائی/اکتوبر)
یا کسی بھی بڑے بنیادی ڈھانچے میں تبدیلی کے فورا. بعد۔ نمونے کے تحت اسٹور کریں
`artifacts/devportal/drills/<YYYYMMDD>/`۔

| ڈرل | اقدامات | ثبوت |
| ----- | ----- | -------- |
| عرف رول بیک مضمون | 1. حالیہ پروڈکشن کے منشور کا استعمال کرتے ہوئے "تعیناتی ناکام" رول بیک کو دہرائیں۔  2۔ ایک بار تحقیقات گزرنے کے بعد دوبارہ پیدا ہونے سے دوبارہ لنک کریں۔  3۔ ڈرل فولڈر میں `portal.manifest.submit.summary.json` اور تحقیقات لاگز کو رجسٹر کریں۔ | `rollback.submit.json` ، تحقیقات آؤٹ پٹ ، اور ٹیسٹ ریلیز ٹیگ۔ |
| مصنوعی توثیق آڈٹ | 1. `npm run probe:portal` اور `npm run probe:tryit-proxy` پیداوار اور اسٹیجنگ کے خلاف چلائیں۔  2. `npm run check:links` اور فائل `build/link-report.json` چلائیں۔  3۔ پینل Grafana کی اسکرین شاٹس/برآمدات کو جانچنے کی کامیابی کی تصدیق کریں۔ | پروب لاگز + `link-report.json` مینی فسٹ فنگر پرنٹ کا حوالہ دیتے ہیں۔ |

دستاویزات/ڈیورل منیجر اور ایس آر ای گورننس ریویو کو ضائع کرنے والی مشقوں کو بڑھاوا دیں ،
چونکہ روڈ میپ میں سہ ماہی عزم کے ثبوت کی ضرورت ہوتی ہے کہ عرف رول بیک اور
پورٹل تحقیقات صحت مند ہیں۔

## پیجریڈی اور آن کال کوآرڈینیشن- پیجریڈی سروس ** دستاویزات پورٹل پبلشنگ ** سے پیدا ہونے والے انتباہات کا مالک ہے
  `dashboards/grafana/docs_portal.json`۔ قواعد `DocsPortal/GatewayRefusals` ،
  `DocsPortal/AliasCache` ، اور `DocsPortal/TLSExpiry` صفحہ برائے دستاویزات/Deverl پرائمری
  سیکنڈری کے طور پر اسٹوریج SRE کے ساتھ.
- جب پیج کیا جائے تو ، `DOCS_RELEASE_TAG` شامل کریں ، پینل Grafana کے اسکرین شاٹس منسلک کریں
  اس سے پہلے واقعے کے نوٹوں میں تحقیقات/لنک چیک کی آؤٹ پٹ کو متاثر کریں اور لنک کریں
  تخفیف کا آغاز کریں۔
- تخفیف کے بعد (رول بیک یا دوبارہ تعی .ن) ، دوبارہ عمل `npm run probe:portal` ،
  `npm run check:links` ، اور تازہ Grafana اسنیپ شاٹس کو میٹرکس دکھا رہا ہے
  ایک بار پھر دہلیز کے اندر۔ تمام شواہد کو پیجرڈیوٹی واقعے سے جوڑیں
  اس کو حل کرنے سے پہلے۔
- اگر ایک ہی وقت میں دو الرٹس فائر کریں (مثال کے طور پر TLS میعاد ختم ہونے کے علاوہ بیک بلاگ)
  پہلے انکار (اشاعت بند کرو) ، پھر رول بیک کے طریقہ کار کو انجام دیں ، پھر
  پل پر اسٹوریج ایس آر ای کے ساتھ ٹی ایل ایس/بیکلاگ آئٹمز کو صاف کرتا ہے۔