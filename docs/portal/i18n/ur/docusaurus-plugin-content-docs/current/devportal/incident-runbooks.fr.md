---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# واقعہ رن بکس اور رول بیک مشقیں

## مقصد

روڈ میپ آئٹم ** دستاویزات -9 ** قابل عمل پلے بوکس کے علاوہ ایک ریہرسل پلان کی درخواست کرتا ہے تاکہ
پورٹل آپریٹرز بغیر کسی اندازہ کے ناکام ترسیل سے بازیافت کرسکتے ہیں۔ یہ نوٹ
تین اعلی سگنل واقعات کا احاطہ کرتا ہے - ناکام تعیناتی ، نقل کی انحطاط اور
تجزیات کی بندش - اور دستاویزات سہ ماہی مشقیں جو عرف رول بیک کو ثابت کرتی ہیں
اور مصنوعی توثیق ہمیشہ اختتام سے آخر میں کام کرتی ہے۔

### متعلقہ مواد

- [`devportal/deploy-guide`] (./deploy-guide) - پیکیجنگ ، دستخط اور عرف پروموشن ورک فلو۔
-.
- `docs/source/sorafs_node_client_protocol.md`
  اور [`sorafs/pin-registry-ops`] (../sorafs/pin-registry-ops)
  - ٹیلی میٹری اور اسکیلیشن دہلیز کو رجسٹر کریں۔
- `docs/portal/scripts/sorafs-pin-release.sh` اور مددگار `npm run probe:*`
  چیک لسٹس میں حوالہ جات۔

### ٹیلی میٹری اور مشترکہ ٹولنگ

| سگنل / ٹول | مقصد |
| --------------- | ------- |
| `torii_sorafs_replication_sla_total` (میٹ/مس/زیر التواء) | نقل کی رکاوٹوں اور ایس ایل اے کی خلاف ورزیوں کا پتہ لگاتا ہے۔ |
| `torii_sorafs_replication_backlog_total` ، `torii_sorafs_replication_completion_latency_epochs` | ٹریج کے لئے بیک بلاگ کی گہرائی اور تکمیل میں تاخیر کی مقدار۔ |
| `torii_sorafs_gateway_refusals_total` ، `torii_sorafs_manifest_submit_total{status="error"}` | گیٹ وے کی طرف کی ناکامیوں کو ظاہر کرتا ہے جو اکثر ناکام تعیناتی کی پیروی کرتے ہیں۔ |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | مصنوعی تحقیقات جو ریلیز کو خراب کرتی ہیں اور رول بیکس کی توثیق کرتی ہیں۔ |
| `npm run check:links` | ٹوٹے ہوئے لنکس کا گیٹ ؛ ہر تخفیف کے بعد استعمال کیا جاتا ہے۔ |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` کے ذریعہ لپیٹا ہوا) | عرف پروموشن/ریجن میکانزم۔ |
| `Docs Portal Publishing` Grafana بورڈ (`dashboards/grafana/docs_portal.json`) | اجتماعی انکار/عرف/ٹی ایل ایس/نقل ٹیلی میٹری۔ پیجریڈی الرٹس ان علامات کو بطور ثبوت حوالہ دیتے ہیں۔ |

## رن بک - ناکام تعیناتی یا ناقص نمونہ

### ٹرگر شرائط

- پیش نظارہ/پیداوار کی تحقیقات ناکام (`npm run probe:portal -- --expect-release=...`)۔
- `torii_sorafs_gateway_refusals_total` پر Grafana کو انتباہات یا
  `torii_sorafs_manifest_submit_total{status="error"}` ایک رول آؤٹ کے بعد۔
- دستی QA نوٹس ٹوٹے ہوئے راستے یا پراکسی بندش کے فورا بعد ہی اسے آزمائیں
  عرف کو فروغ دینا۔

### فوری قید

1. ** منجمد تعیناتی: ** CI پائپ لائن کو `DEPLOY_FREEZE=1` کے ساتھ نشان زد کریں (ورک فلو ان پٹ
   گٹ ہب) یا جینکنز کی نوکری کو روکیں تاکہ کوئی نمونے نہیں چھوڑیں۔
2.
   `portal.manifest*.{json,to,bundle,sig}` ، اور ناکام تعمیراتی تحقیقات کی پیداوار تاکہ
   رول بیک ٹھیک ہضموں کا حوالہ دیتا ہے۔
3. ** اسٹیک ہولڈرز کو مطلع کریں: ** اسٹوریج ایس آر ای ، لیڈ دستاویزات/ڈیوریل ، اور ڈیوٹی آفیسر
   بیداری کے لئے گورننس (خاص طور پر اگر `docs.sora` متاثر ہوا ہے)۔

### رول بیک طریقہ کار

1. آخری معروف نیک (LKG) کے ظاہر کی شناخت کریں۔ پروڈکشن ورک فلو ان کے تحت اسٹور کرتا ہے
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`۔
2. شپنگ مددگار کے ساتھ عرف کو اس ظاہر سے دوبارہ لنک کریں:

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

3. رول بیک کے ہضم کے ساتھ واقعہ کے ٹکٹ میں رول بیک کا خلاصہ محفوظ کریں
   ایل کے جی منشور اور ناکام مینی فیسٹ۔

### توثیق1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` اور `sorafs_cli proof verify ...`
   ۔
   پھر بھی کار آرکائیو میں۔
4. `npm run probe:tryit-proxy` اس بات کو یقینی بنانے کے لئے کہ کوشش کی جانے والی پراکسی واپس آگئی ہے۔

### بعد کے بعد

1. بنیادی وجہ کو سمجھنے کے بعد ہی تعیناتی پائپ لائن کو دوبارہ متحرک کریں۔
2. [`devportal/deploy-guide`] (./deploy-guide) میں "سیکھے گئے" اندراجات کو اپ ڈیٹ کریں
   نئے نکات کے ساتھ ، اگر ضروری ہو تو۔
3. ناکام ٹیسٹ سویٹ (تحقیقات ، لنک چیکر ، وغیرہ) کے لئے کھلے نقائص۔

## رن بک - نقل کی ہراس

### ٹرگر شرائط

- الرٹ: `رقم (torii_sorafs_replication_sla_total {نتیجہ =" میٹ "}) /
  کلیمپ_مین (رقم (torii_sorafs_replication_sla_total {نتیجہ = ~ "met | med | med"}) ، 1)  15 منٹ کے لئے۔
- رازداری کے جائزے سے ترک شدہ واقعات میں غیر متوقع اضافہ کا پتہ چلتا ہے۔
- `npm run probe:tryit-proxy` راستوں پر ناکام ہوتا ہے `/probe/analytics`۔

### جواب1. تعمیر اندراجات کی جانچ کریں: `DOCS_ANALYTICS_ENDPOINT` اور
   `DOCS_ANALYTICS_SAMPLE_RATE` ریلیز آرٹیکٹیکٹ (`build/release.json`) میں۔
2. `DOCS_ANALYTICS_ENDPOINT` کے ساتھ `npm run probe:portal` کو دوبارہ ایکسٹیٹ کریں
   اس بات کی تصدیق کرنے کے لئے کلیکٹر کا اسٹیج کرنا کہ ٹریکر ابھی بھی پے لوڈ بھیج رہا ہے۔
3. اگر جمع کرنے والے نیچے ہیں تو ، `DOCS_ANALYTICS_ENDPOINT=""` سیٹ کریں اور دوبارہ تعمیر کریں
   کہ ٹریکر شارٹ سرکٹس ؛ ٹائم لائن پر آؤٹ آؤٹ ونڈو کو لاگ ان کریں۔
4. توثیق کریں کہ `scripts/check-links.mjs` فنگر پرنٹ `checksums.sha256` پر جاری ہے
   (تجزیات کی بندش کو * نہیں * سائٹ کا نقشہ کی توثیق کرنا چاہئے)۔
5. ایک بار جب کلکٹر کو بحال کردیا گیا ہے ، عمل کرنے کے لئے `npm run test:widgets` چلائیں
   یونٹ دوبارہ شائع ہونے سے پہلے تجزیاتی مددگار کی جانچ کرتا ہے۔

### بعد کے بعد

1. اپ ڈیٹ [`devportal/observability`] (./observability) نئی حدود کے ساتھ
   کلکٹر یا نمونے لینے کی ضروریات۔
2۔ گورننس کا نوٹس جاری کریں اگر تجزیات کا ڈیٹا ضائع ہوچکا ہے یا اسے دوبارہ تیار کیا گیا ہے
   سیاست سے باہر

## سہ ماہی لچکدار مشقیں

ہر سہ ماہی کے ** پہلے منگل کے دوران دونوں مشقیں چلائیں ** (جنوری/اپریل/جولائی/اکتوبر)
یا کسی بھی بڑے بنیادی ڈھانچے میں تبدیلی کے فورا. بعد۔ نمونے کے تحت اسٹور کریں
`artifacts/devportal/drills/<YYYYMMDD>/`۔

| ڈرل | اقدامات | ثبوت |
| ----- | ----- | -------- |
| دہرائیں عرف رول بیک | 1. حالیہ پروڈکشن کے ظاہر کے ساتھ "تعیناتی کی شرح" رول بیک کو دوبارہ چلائیں۔  2۔ ایک بار تحقیقات گزرنے کے بعد دوبارہ پیدا ہونے سے دوبارہ لنک کریں۔  3۔ ڈرل فولڈر میں `portal.manifest.submit.summary.json` اور تحقیقات کے نوشتہ جات کو بچائیں۔ | `rollback.submit.json` ، تحقیقات کی پیداوار ، اور تکرار کا ٹیگ۔ |
| مصنوعی توثیق آڈٹ | 1. لانچ `npm run probe:portal` اور `npm run probe:tryit-proxy` پیداوار اور اسٹیجنگ کے خلاف۔  2۔ `npm run check:links` اور آرکائیو `build/link-report.json` لانچ کریں۔  3۔ پینل Grafana کی اسکرین شاٹس/برآمدات جو تحقیقات کی کامیابی کی تصدیق کرتے ہیں۔ | تحقیقات لاگز + `link-report.json` مینی فیسٹ کے فنگر پرنٹ کا حوالہ دیتے ہیں۔ |

دستاویزات/ڈیوریل منیجر اور گورننس ایس آر ای جائزہ سے محروم مشقوں کو بڑھاوا دیں ، کیونکہ
روڈ میپ کے لئے سہ ماہی عزم کا ثبوت درکار ہوتا ہے کہ عرف رول بیک اور تحقیقات
پورٹل صحت مند رہتا ہے۔

## پیجریڈی اور آن کال کوآرڈینیشن- پیجریڈی ** دستاویزات پورٹل پبلشنگ ** سروس کے بعد سے انتباہات پیدا ہوئے ہیں
  `dashboards/grafana/docs_portal.json`۔ قواعد `DocsPortal/GatewayRefusals` ،
  `DocsPortal/AliasCache` ، اور `DocsPortal/TLSExpiry` صفحہ بنیادی دستاویزات/deverl
  سیکنڈری کے طور پر اسٹوریج SRE کے ساتھ.
- جب صفحے پر ، `DOCS_RELEASE_TAG` شامل کریں ، پینلز کے اسکرین شاٹس منسلک کریں
  Grafana اثرات اور بائنڈ تحقیقات/لنک چیک آؤٹ پٹ فارورڈ واقعہ نوٹس میں
  تخفیف شروع کرنے کے لئے.
- تخفیف کے بعد (رول بیک یا دوبارہ تعی .ن) ، دوبارہ عمل `npm run probe:portal` ،
  `npm run check:links` ، اور اسنیپ شاٹس Grafana کو میٹرکس دکھا رہا ہے
  دہلیز پر لوٹ آیا۔ تمام شواہد کو پیجرڈیوٹی واقعے سے جوڑیں
  قرارداد سے پہلے
- اگر ایک ہی وقت میں دو الرٹس کو متحرک کیا جاتا ہے (جیسے TLS میعاد ختم ہونے والے پلس بیکلاگ) ، ترتیب دیں
  پہلے (اشاعت کو روکنے) سے انکار کرتا ہے ، پھر رول بیک طریقہ کار پر عملدرآمد کریں ، پھر
  پل پر اسٹوریج SRE کے ساتھ TLS/بیکلاگ پر کارروائی کریں۔