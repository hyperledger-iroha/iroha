---
lang: ur
direction: rtl
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T15:38:30.658489+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! خفیہ اثاثوں کا آڈٹ اور آپریشنز پلے بوک `roadmap.md:M4` کے ذریعہ حوالہ دیا گیا ہے۔

# خفیہ اثاثے آڈٹ اور آپریشنز رن بک

یہ گائیڈ شواہد کو مستحکم کرتا ہے آڈیٹرز اور آپریٹرز پر انحصار کرتے ہیں
جب خفیہ اثاثہ بہاؤ کی توثیق کرتے ہو۔ یہ گردش پلے بک کو پورا کرتا ہے
(`docs/source/confidential_assets_rotation.md`) اور انشانکن لیجر
(`docs/source/confidential_assets_calibration.md`)۔

## 1۔ انتخابی انکشاف اور ایونٹ فیڈز

- ہر خفیہ ہدایت ایک ساختی `ConfidentialEvent` پے لوڈ کا اخراج کرتی ہے
  (`Shielded` ، `Transferred` ، `Unshielded`)
  `crates/iroha_data_model/src/events/data/events.rs:198` اور بذریعہ سیریلائزڈ
  ایگزیکٹو (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699` - `4021`)۔
  ریگریشن سویٹ کنکریٹ پے لوڈ کو استعمال کرتا ہے تاکہ آڈیٹر انحصار کرسکیں
  عین مطابق JSON لے آؤٹ (`crates/iroha_core/tests/zk_confidential_events.rs:19` - `299`)۔
- Torii ان واقعات کو معیاری SSE/ویب ساکٹ پائپ لائن کے ذریعے بے نقاب کرتا ہے۔ آڈیٹر
  `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`) کا استعمال کرتے ہوئے سبسکرائب کریں ،
  اختیاری طور پر ایک ہی اثاثہ تعریف کو ختم کرنا۔ سی ایل آئی مثال:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- پالیسی میٹا ڈیٹا اور زیر التوا ٹرانزیشن دستیاب ہیں
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`) ، سوئفٹ SDK کے ذریعہ آئینہ دار
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) اور دستاویزی دستاویز
  دونوں خفیہ اثاثوں کے ڈیزائن اور ایس ڈی کے گائڈز
  (`docs/source/confidential_assets.md:70` ، `docs/source/sdk/swift/index.md:334`)۔

## 2۔ ٹیلی میٹری ، ڈیش بورڈز ، اور انشانکن ثبوت

- رن ٹائم میٹرکس سطح کے درخت کی گہرائی ، عزم/فرنٹیئر ہسٹری ، روٹ بے دخلی
  کاؤنٹرز ، اور تصدیق کنندہ کیچ ہٹ تناسب
  (`crates/iroha_telemetry/src/metrics.rs:5760` - `5815`)۔ Grafana ڈیش بورڈز میں
  `dashboards/grafana/confidential_assets.json` سے وابستہ پینل اور جہاز بھیج دیں
  `docs/source/confidential_assets.md:401` میں دستاویزی ورک فلو کے ساتھ انتباہات۔
- انشانکن رنز (این ایس/او پی ، گیس/او پی ، این ایس/گیس) دستخط شدہ لاگ ان کے ساتھ براہ راست
  `docs/source/confidential_assets_calibration.md`۔ تازہ ترین ایپل سلیکن
  نیین رن پر محفوظ شدہ دستاویزات ہیں
  `docs/source/confidential_assets_calibration_neon_20260428.log` ، اور ایک ہی
  لیجر نے سم ڈی غیر جانبدار اور اے وی ایکس 2 پروفائلز کے لئے عارضی چھوٹ کو ریکارڈ کیا جب تک
  x86 میزبان آن لائن آتے ہیں۔

## 3. واقعہ کا جواب اور آپریٹر کے کام

- گردش/اپ گریڈ کے طریقہ کار میں رہتے ہیں
  `docs/source/confidential_assets_rotation.md` ، جس میں نیا اسٹیج کرنے کا طریقہ ہے
  پیرامیٹر بنڈل ، شیڈول پالیسی اپ گریڈ ، اور بٹوے/آڈیٹرز کو مطلع کریں۔
  ٹریکر (`docs/source/project_tracker/confidential_assets_phase_c.md`) فہرستیں
  رن بک مالکان اور ریہرسل توقعات۔
- پروڈکشن ریہرسلز یا ہنگامی ونڈوز کے لئے ، آپریٹرز ثبوت سے منسلک ہوتے ہیں
  `status.md` اندراجات (جیسے ، ملٹی لین ریہرسل لاگ) اور اس میں شامل ہیں:
  `curl` پالیسی ٹرانزیشن کا ثبوت ، Grafana اسنیپ شاٹس ، اور متعلقہ واقعہ
  ڈائجسٹس تاکہ آڈیٹر ٹکسال → منتقلی → ظاہر کردہ ٹائم لائنز کی تشکیل نو کرسکیں۔

## 4۔ بیرونی جائزہ کیڈینس

- سیکیورٹی جائزہ دائرہ کار: خفیہ سرکٹس ، پیرامیٹر رجسٹری ، پالیسی
  ٹرانزیشن ، اور ٹیلی میٹری۔ اس دستاویز کے علاوہ انشانکن لیجر فارم
  ثبوتوں کا پیکٹ فروشوں کو بھیجا گیا۔ جائزہ شیڈولنگ کے ذریعے ٹریک کیا جاتا ہے
  M4 `docs/source/project_tracker/confidential_assets_phase_c.md` میں۔
- آپریٹرز کو کسی بھی وینڈر کے نتائج یا فالو اپ کے ساتھ `status.md` کو اپ ڈیٹ رکھنا چاہئے
  ایکشن آئٹمز۔ جب تک بیرونی جائزہ مکمل نہیں ہوتا ، یہ رن بک رب کے طور پر کام کرتی ہے
  آپریشنل بیس لائن آڈیٹر اس کے خلاف جانچ کر سکتے ہیں۔