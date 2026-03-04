---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# واقعہ رن بکس اور رول بیک مشقیں

## مقصد

روڈ میپ آئٹم ** دستاویزات -9 ** کے لئے قابل عمل پلے بوکس کے علاوہ ایک ریہرسل پلان کی ضرورت ہے تاکہ
پورٹل آپریٹرز بغیر کسی تخمینے کے ناکامی بھیجنے کی بازیافت کرسکتے ہیں۔ اس نوٹ میں تین کا احاطہ کیا گیا ہے
اعلی سگنل کے واقعات - ناکام تعیناتی ، نقل کی انحطاط اور تجزیات کی بندش - اور
دستاویزات سہ ماہی مشقیں جو عرف رول بیک اور مصنوعی توثیق کو ثابت کرتی ہیں
کام ختم ہونے کے لئے جاری رکھیں۔

### متعلقہ مواد

- [`devportal/deploy-guide`] (./deploy-guide) - پیکیجنگ ، دستخط اور عرف پروموشن ورک فلو۔
-.
- `docs/source/sorafs_node_client_protocol.md`
  اور [`sorafs/pin-registry-ops`] (../sorafs/pin-registry-ops)
  - رجسٹری ٹیلی میٹری اور بڑھتی ہوئی حدود۔
- `docs/portal/scripts/sorafs-pin-release.sh` اور `npm run probe:*` مددگار
  چیک لسٹس میں حوالہ دیا گیا۔

### مشترکہ ٹیلی میٹری اور ٹولنگ

| سگنل / ٹول | مقصد |
| --------------- | ------- |
| `torii_sorafs_replication_sla_total` (میٹ/مس/زیر التواء) | نقل کے بلاکس اور ایس ایل اے کی خلاف ورزیوں کا پتہ لگاتا ہے۔ |
| `torii_sorafs_replication_backlog_total` ، `torii_sorafs_replication_completion_latency_epochs` | ٹریج کے لئے بیک بلاگ کی گہرائی اور تکمیل میں تاخیر کی مقدار۔ |
| `torii_sorafs_gateway_refusals_total` ، `torii_sorafs_manifest_submit_total{status="error"}` | گیٹ وے کی ناکامیوں کو ظاہر کرتا ہے جو اکثر خراب تعیناتی کی پیروی کرتے ہیں۔ |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | مصنوعی تحقیقات جو گیٹ رول بیکس کو جاری کرتی ہے اور اس کی توثیق کرتی ہے۔ |
| `npm run check:links` | ٹوٹا ہوا لنک گیٹ ؛ ہر تخفیف کے بعد استعمال کیا جاتا ہے۔ |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` کے ذریعہ استعمال کیا جاتا ہے) | عرف پروموشن/ریجن میکانزم۔ |
| `Docs Portal Publishing` Grafana بورڈ (`dashboards/grafana/docs_portal.json`) | اجتماعی انکار/عرف/ٹی ایل ایس/نقل ٹیلی میٹری۔ پیجریڈی الرٹس ان پینلز کو بطور ثبوت حوالہ دیتے ہیں۔ |

## رن بک - ناکام تعیناتی یا خراب نمونے

### ٹرگر شرائط

- پیش نظارہ/پیداوار کی تحقیقات ناکام (`npm run probe:portal -- --expect-release=...`)۔
- `torii_sorafs_gateway_refusals_total` پر Grafana کو انتباہات یا
  `torii_sorafs_manifest_submit_total{status="error"}` ایک رول آؤٹ کے بعد۔
- دستی QA نوٹ ٹوٹے ہوئے راستوں یا پراکسی کی ناکامیوں کے فورا بعد ہی اسے آزمائیں
  عرف کی تشہیر۔

### فوری طور پر کنٹینمنٹ

1. ** منجمد تعیناتی: ** CI پائپ لائن کو `DEPLOY_FREEZE=1` کے ساتھ نشان زد کریں (ورک فلو ان پٹ
   گٹ ہب) یا جینکنز کی نوکری کو روکیں تاکہ کسی نمونے کو نہیں دھکیل دیا جائے۔
2.
   `portal.manifest*.{json,to,bundle,sig}` ، اور ناکام تعمیر سے تحقیقات کی پیداوار تاکہ
   رول بیک ٹھیک ہضموں کا حوالہ دیتا ہے۔
3. ** اسٹیک ہولڈرز کو مطلع کریں: ** اسٹوریج ایس آر ای ، لیڈ دستاویزات/ڈیوریل ، اور ڈیوٹی آفیسر
   بیداری کے لئے گورننس (خاص طور پر جب `docs.sora` متاثر ہوتا ہے)۔

### رول بیک طریقہ کار

1. آخری معروف نیک (LKG) کے ظاہر کی شناخت کریں۔ پروڈکشن ورک فلو ان میں ذخیرہ کرتا ہے
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

3. واقعہ کے ہضموں کے ساتھ ساتھ واقعے کے ٹکٹ میں رول بیک کا خلاصہ ریکارڈ کریں
   منشور ایل کے جی اور ناکام مینی فیسٹ۔

### توثیق1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` اور `sorafs_cli proof verify ...`
   (تعیناتی گائیڈ ملاحظہ کریں) اس بات کی تصدیق کرنے کے لئے کہ دوبارہ پیش کردہ مظہر جاری ہے
   آرکائیو کار کے ساتھ پیٹ رہا ہے۔
4. `npm run probe:tryit-proxy` اس بات کو یقینی بنانے کے لئے کہ کوشش کریں کہ اسٹیجنگ پراکسی واپس آگیا ہے۔

### بعد کے بعد

1. بنیادی وجہ کو سمجھنے کے بعد ہی تعیناتی پائپ لائن کو دوبارہ متحرک کریں۔
2. [`devportal/deploy-guide`] (./deploy-guide) میں "سبق سیکھے گئے" اندراجات کو پُر کریں
   نئے نکات کے ساتھ ، اگر کوئی ہے۔
3. ناکام ٹیسٹ سویٹ (تحقیقات ، لنک چیکر ، وغیرہ) کے لئے کھلے نقائص۔

## رن بک - نقل کی ہراس

### ٹرگر شرائط

- الرٹ: `رقم (torii_sorafs_replication_sla_total {نتیجہ =" میٹ "}) /
  کلیمپ_مین (رقم (torii_sorafs_replication_sla_total {نتیجہ = ~ "met | med | med"}) ، 1)  15 منٹ کے لئے۔
- رازداری کا جائزہ ضائع ہونے والے واقعات میں غیر متوقع اضافے کی نشاندہی کرتا ہے۔
- `npm run probe:tryit-proxy` راستوں پر ناکام ہوتا ہے `/probe/analytics`۔

### جواب1. بلڈ ان پٹ چیک کریں: `DOCS_ANALYTICS_ENDPOINT` اور
   `DOCS_ANALYTICS_SAMPLE_RATE` ریلیز آرٹیکٹیکٹ (`build/release.json`) میں۔
2. `DOCS_ANALYTICS_ENDPOINT` کے ساتھ `npm run probe:portal` RERUN کی طرف اشارہ کرتے ہوئے
   اس بات کی تصدیق کرنے کے لئے کلیکٹر کا اسٹیج کرنا کہ ٹریکر ابھی بھی پے لوڈ کا اخراج کررہا ہے۔
3. اگر جمع کرنے والے نیچے ہیں تو ، `DOCS_ANALYTICS_ENDPOINT=""` سیٹ کریں اور دوبارہ تعمیر کریں
   تاکہ ٹریکر شارٹ سرکٹس ؛ لائن پر آؤٹ پٹ ونڈو کو ریکارڈ کریں
   واقعے کا وقت۔
4. توثیق کریں کہ `scripts/check-links.mjs` اسٹیل فنگر پرنٹ `checksums.sha256`
   (تجزیات کے قطرے کو * نہیں * سائٹ کا نقشہ کی توثیق کرنا چاہئے)۔
5. جب کلکٹر لوٹتا ہے تو ، یونٹ ٹیسٹ انجام دینے کے لئے `npm run test:widgets` چلائیں
   دوبارہ شائع ہونے سے پہلے تجزیاتی مددگار سے۔

### بعد کے بعد

1. اپ ڈیٹ [`devportal/observability`] (./observability) نئی کلکٹر کی حدود کے ساتھ
   یا نمونے لینے کی ضروریات۔
2۔ گورننس کا نوٹس کھولیں اگر تجزیات کا ڈیٹا ضائع ہوا یا اس کو دوبارہ تیار کیا گیا ہو
   سیاست کی

## سہ ماہی لچکدار مشقیں

ہر سہ ماہی کے ** پہلے منگل کے دوران دونوں مشقیں چلائیں ** (جنوری/اپریل/جولائی/اکتوبر)
یا کسی بھی بڑے بنیادی ڈھانچے میں تبدیلی کے فورا. بعد۔ میں نمونے اسٹور کریں
`artifacts/devportal/drills/<YYYYMMDD>/`۔

| ڈرل | اقدامات | ثبوت |
| ----- | ----- | -------- |
| عرف رول بیک ٹیسٹ | 1۔ تازہ ترین پروڈکشن مینی فیسٹ کا استعمال کرتے ہوئے "ناکام تعینات" رول بیک پر دوبارہ کوشش کریں۔  2۔ جب بھی تحقیقات گزریں تو پیداوار کو دوبارہ لنک کریں۔  3۔ `portal.manifest.submit.summary.json` لاگ ان کریں اور ڈرل فولڈر میں لاگ ان کی تحقیقات کریں۔ | `rollback.submit.json` ، تحقیقات آؤٹ پٹ ، اور پرکھ ریلیز ٹیگ۔ |
| مصنوعی توثیق آڈٹ | 1. `npm run probe:portal` اور `npm run probe:tryit-proxy` پیداوار اور اسٹیجنگ کے خلاف چلائیں۔  2. `npm run check:links` اور فائل `build/link-report.json` چلائیں۔  3۔ Grafana پینلز کی اسکرین شاٹس/برآمدات جو تحقیقات کی کامیابی کی تصدیق کرتے ہیں۔ | تحقیقات لاگز + `link-report.json` مینی فسٹ فنگر پرنٹ کا حوالہ دیتے ہیں۔ |

دستاویزات/ڈیورل منیجر اور ایس آر ای گورننس ریویو کو ضائع کرنے والی مشقوں کو بڑھاوا دیں ،
کیونکہ روڈ میپ کے لئے عرف رول بیک اور عرف رول بیک اور اس کے لئے عصبی سہ ماہی شواہد کی ضرورت ہوتی ہے
پورٹل تحقیقات صحت مند ہیں۔

## پیجریڈی اور آن کال کوآرڈینیشن- پیجریڈی ** دستاویزات پورٹل پبلشنگ ** سروس سے حاصل کردہ انتباہات کا مالک ہے
  `dashboards/grafana/docs_portal.json`۔ قواعد `DocsPortal/GatewayRefusals` ،
  `DocsPortal/AliasCache` ، اور `DocsPortal/TLSExpiry` صفحہ پرائمری دستاویزات/ڈیوریل سے
  سیکنڈری کے طور پر اسٹوریج SRE کے ساتھ.
- جب صفحہ چلتا ہے تو ، `DOCS_RELEASE_TAG` کو شامل کریں ، Grafana پینلز کے اسکرین شاٹس منسلک کریں
  شروع کرنے سے پہلے واقعے کے نوٹوں میں متاثرہ اور تحقیقات/لنک چیک آؤٹ پٹ کو لنک کریں
  تخفیف
- تخفیف کے بعد (رول بیک یا دوبارہ تعی .ن) ، `npm run probe:portal` کو دوبارہ بنائیں ،
  `npm run check:links` ، اور حالیہ Grafana اسنیپ شاٹس کو میٹرکس دکھا رہا ہے
  دہلیز پر واپس تمام شواہد کو پیجرڈیوٹی واقعے سے جوڑیں
  حل کرنے سے پہلے
اگر ایک ہی وقت میں دو الرٹس فائر کریں (مثال کے طور پر TLS میعاد ختم ہونے کے علاوہ بیک بلاگ) ،
  ٹریج سے انکار پہلے (پبلشنگ کو روکیں) ، رول بیک کا طریقہ کار انجام دیں اور
  پھر پل پر اسٹوریج SRE کے ساتھ TLS/بیکلاگ حل کریں۔