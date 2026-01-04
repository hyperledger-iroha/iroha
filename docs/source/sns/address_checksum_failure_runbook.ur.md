---
lang: ur
direction: rtl
source: docs/source/sns/address_checksum_failure_runbook.md
status: complete
translator: Codex (automated)
source_hash: 7ea5de87a9a88539944ecbfbd7f5ba176939446d70436ec073c8bb103fd9494f
source_last_modified: "2025-11-15T15:12:42.620964+00:00"
translation_last_reviewed: 2025-11-18
---

<div dir="rtl">

<!-- docs/source/sns/address_checksum_failure_runbook.md کا اردو ترجمہ -->

# اکاؤنٹ ایڈریس چیک سم انسیڈنٹ رن بک (ADDR-7)

حالت: 23 اپریل 2026 کو نافذ  
مالکان: Torii پلیٹ فارم ٹیم / SRE / والٹ اور ایکسپلورر سپورٹ  
روڈ میپ لنک: **ADDR-6/ADDR-7 — UX اور سیکیورٹی ایکسیپٹینس معیارات**

یہ رن بک اس آپریشنل ردِعمل کی وضاحت کرتی ہے جب Torii، SDKs، یا والٹ سطح پر
IH58/کمپریسڈ چیک سم فیلیرز (`ERR_CHECKSUM_MISMATCH` / `ChecksumMismatch`) رپورٹ
ہوں۔ یہ
[`address_security_review.md`](./address_security_review.md)،
[`address_display_guidelines.md`](./address_display_guidelines.md)، اور
[`address_manifest_ops.md`](../runbooks/address_manifest_ops.md)
میں موجود طریقۂ کار کی تکمیل کرتا ہے۔

## 1. یہ play کب چلانا ہے

درج ذیل میں سے کوئی بھی واقعہ ہوتے ہی اس رن بک کو ٹرگر کریں:

1. **الرٹینگ:** `AddressInvalidRatioSlo`
   (`dashboards/alerts/address_ingest_rules.yml`) فائر ہو اور
   `torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}`
   اینوٹیشن یا Grafana `address_ingest.json` پینلز میں ظاہر ہو۔
2. **فکسچر گارڈ ریلز:** `ci/account_fixture_metrics.sh` یا
   `account_address_fixture_status.json` ڈیش بورڈ کسی ٹارگٹ کے لیے
   `ChecksumMismatch` لیبل دکھائے۔
3. **سپورٹ/اسکیلیشن:** والٹ، ایکسپلورر، یا SDK ٹیمیں `ERR_CHECKSUM_MISMATCH`,
   `ChecksumMismatch`, یا IME/NFC سے متعلق چیک سم ڈرفٹ کے ٹکٹس فائل کریں۔
4. **دستی دریافت:** Torii لاگز میں `address_parse_error=checksum_mismatch`
   (دیکھیں `crates/iroha_torii/src/utils.rs:439`) بار بار نظر آئے، خاص طور پر
   جب اثر متعدد endpoints یا dataspace کانٹیکسٹس میں ہو۔

اگر انسیڈنٹ Local‑8 یا Local‑12 ڈائجسٹ کولیشن تک محدود ہو تو
`AddressLocal8Resurgence` یا `AddressLocal12Collision` playbooks دیکھیں؛ دیگر
صورت میں نیچے دیے گئے مراحل پر عمل کریں۔

## 2. مشاہدہ اور شواہد کی چیک لسٹ

| ثبوت | مقام / کمانڈ | نوٹس |
|------|--------------|-------|
| Grafana اسنیپ شاٹ | `dashboards/grafana/address_ingest.json` | invalid وجوہ کا بریک ڈاؤن، endpoint پینلز، اور `torii_address_invalid_total` کا گراف (`reason="ERR_CHECKSUM_MISMATCH"`) محفوظ کریں۔ |
| الرٹ کا سیاق | `dashboards/alerts/address_ingest_rules.yml` | فائر ہونے والا الرٹ، PagerDuty/Slack ack، اور context لیبل شامل کریں۔ |
| فکسچر صحت | `dashboards/grafana/account_address_fixture_status.json` + `artifacts/account_fixture/address_fixture.prom` | تصدیق کہ SDK کی کاپی `fixtures/account/address_vectors.json` سے نہیں ہٹی۔ |
| PromQL | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | متاثرہ Torii endpoint کی نشاندہی اور CSV ایکسپورٹ۔ |
| لاگز | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` | ہر کانٹیکسٹ کے کم از کم 5 نمونے، PII ہٹا کر۔ |
| فکسچر ویری فیکیشن | `cargo xtask address-vectors --verify` | یقینی بنائے کہ کینو نیکل فکسچر کمیٹیڈ JSON سے میل کھاتا ہے۔ |
| SDK parity | `python3 scripts/account_fixture_helper.py check --target <path> ...` | الرٹس میں مذکور ہر SDK کے لیے چلائیں؛ بنے ہوئے `address_fixture.prom` کو ٹکٹ میں لگائیں۔ |
| Clipboard/IME sanity | `iroha address inspect <literal>` | مسئلہ والے strings میں چھپے حروف/IME تبدیلیاں ڈھونڈیں؛ `address_display_guidelines.md` کے مطابق تصدیق کریں۔ |

## 3. فوری ردِعمل کے مراحل

1. **الرٹ کی توثیق کریں** اور Grafana + PromQL آؤٹ پٹ انسیڈنٹ تھریڈ میں پوسٹ کریں؛
   متاثرہ `context` لیبلز نوٹ کریں۔
2. **خطرناک ڈپلائز کو روکیں:** جب تک سبب واضح نہ ہو، manifest promotion اور
   address parsing سے وابستہ SDK ریلیزز معطل رکھیں۔
3. **ٹیلی میٹری اسنیپ شاٹس** (`address_ingest.json`, `account_address_fixture_status.json`)
   ڈاؤن لوڈ کر کے انسیڈنٹ فولڈر (`docs/source/sns/incidents/YYYY-MM/<ticket>/`)
   میں رکھیں۔
4. **لاگ نمونے اکٹھے کریں:** `journalctl` یا لاگ ایگریگیٹر سے نمایاں
   `checksum_mismatch` اندراجات نکالیں اور شیئر کرنے سے پہلے ایڈریس ریڈیکٹ کریں۔
5. **SDK مالکان کو خبردار کریں:** `#sdk-parity` چینل میں JS/Android/Swift/Python
   لیڈز کو ٹیگ کریں اور نمونہ payloads شیئر کریں۔

## 4. وجہ کی علیحدگی

### 4.1 فکسچر یا جنریٹر ڈرفٹ

- `cargo xtask address-vectors --verify` چلائیں؛ ناکامی کی صورت میں فکسچر دوبارہ
  بنائیں اور اسے ڈیٹا ماڈل پائپ لائن regression سمجھیے۔
- الرٹس میں مذکور ہر SDK کے لیے `scripts/account_fixture_helper.py fetch` +
  `check` (یا `ci/account_fixture_metrics.sh`) چلائیں اور بنی ہوئی
  `address_fixture.prom` فائل ٹکٹ میں شامل کریں۔

### 4.2 کلائنٹ انکوڈر/IME ریگریشنز

- `iroha address inspect <literal>` کے ذریعے اصل payload بحال کریں اور
  زیرو وِد یا kana تبدیلیاں تلاش کریں۔ `address_display_guidelines.md` اور
  ADDR‑6 کے QR/ڈسپلے اصولوں کے خلاف نفاذ کی تصدیق کریں۔
- اگر clipboard/کیمرہ ingestion وجہ ہو تو والٹس کے QR ہیلپر endpoints بدستور
  استعمال ہو رہے ہوں۔

### 4.3 Manifest یا رجسٹری خرابیاں

- اگر مسئلہ صرف ایک suffix/domain تک محدود ہو تو
  [`address_manifest_ops.md`](../runbooks/address_manifest_ops.md)
  کے مطابق تازہ ترین manifest بنڈل کو جانچیں۔
- `scripts/address_local_toolkit.sh audit --input <file>` (دستاویز:
  شامل نہ ہو۔

### 4.4 بدنیتی پر مبنی یا خراب ٹریفک

- Torii لاگز اور `torii_http_requests_total` سے معلوم کریں کہ درخواستیں کس
  IP/app ID سے آ رہی ہیں اور ضرورت ہو تو rate-limit یا بلاک کریں۔
- سیکیورٹی/گورننس کے لیے کم از کم 24 گھنٹے کے لاگز محفوظ رکھیں۔

## 5. اصلاح اور بحالی

| منظر | اقدامات |
|------|---------|
| فکسچر ڈرفٹ | `fixtures/account/address_vectors.json` دوبارہ بنائیں، `cargo xtask address-vectors --verify` اور `ci/account_fixture_metrics.sh` چلائیں، اور SDK ریلیزز کو تازہ بنڈل پر منتقل کریں۔ تبدیلی کو `status.md` اور گورننس ٹریکر میں نوٹ کریں۔ |
| SDK/کلائنٹ ریگریشن | متاثرہ SDK کے خلاف بگ فائل کریں، کینو نیکل فکسچر اور `iroha address inspect` آؤٹ پٹ کا حوالہ دیں، اور ریلیز کو `ci/check_address_normalize.sh` یا متعلقہ parity جاب کے پیچھے گیٹ کریں۔ |
| Manifest کرپشن | Manifest رن بک کے مطابق بنڈل دوبارہ تیار کریں، `cargo xtask address-manifest verify` چلائیں، اور جب تک ٹیلی میٹری مستحکم نہ ہو `torii.strict_addresses=true` فعال رکھیں۔ |
| بدنیتی ٹریفک | Torii سطح پر JWT/app IDs فلٹر کریں، گیٹ وے پر تھروٹل لگائیں، اور خراب ایڈریسز کو گورننس انسیڈنٹ ٹریکر میں درج کریں تاکہ ضرورت پڑنے پر Local سلیکٹر tombstone کیے جا سکیں۔ |

اصلاحات کے بعد PromQL کو دوبارہ چلائیں تاکہ `ERR_CHECKSUM_MISMATCH` کم از کم
30 منٹ کے لیے صفر رہے۔

## 6. اختتامی چیک لسٹ

1. Grafana اسنیپ شاٹس، PromQL CSV، `address_fixture.prom`, اور لاگ اقتباسات
   انسیڈنٹ ریکارڈ کے ساتھ محفوظ کریں۔
2. `status.md` (ADDR سیکشن) اور روڈ میپ قطار کو اپ ڈیٹ کریں اگر ٹولنگ/دستاویز
   بدلی ہو۔
3. `docs/source/sns/incidents/` میں مختصر پوسٹ انسیڈنٹ نوٹ شامل کریں (جب نئی
   سیکھ ملے) اور اسے گورننس ٹریکر سے جوڑیں۔
4. اگر SDK فکسز درکار ہوں تو ان کے ریلیز نوٹس میں checksum انسیڈنٹ کا حوالہ دیں
   اور commit + CI ثبوت منسلک کریں۔
5. الرٹ تبھی بند کریں جب
   `torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}`
   (سوائے `/tests/*`) 24 گھنٹے تک صفر رہے اور تمام فکسچر چیکس سبز ہوں۔

اس رن بک پر عمل کرنے سے ADDR‑6/ADDR‑7 کے تقاضے پورے رہتے ہیں اور checksum
فیلیرز کو قابلِ تکرار شواہد کے ساتھ سنبھالا جا سکتا ہے۔

</div>
