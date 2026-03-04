---
lang: ur
direction: rtl
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T15:38:30.658859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! خفیہ اثاثہ گردش پلے بک کا حوالہ `roadmap.md:M3` کے ذریعہ کیا گیا ہے۔

# خفیہ اثاثہ گردش رن بک

یہ پلے بوک وضاحت کرتا ہے کہ آپریٹرز خفیہ اثاثہ کا شیڈول اور اس پر عمل کرتے ہیں
گردش (پیرامیٹر سیٹ ، چابیاں کی تصدیق ، اور پالیسی ٹرانزیشن) جبکہ
بٹوے کو یقینی بنانا ، Torii کلائنٹ ، اور میمپول گارڈز تعصب پسند ہیں۔

## لائف سائیکل اور اسٹیٹس

خفیہ پیرامیٹر سیٹ (`PoseidonParams` ، `PedersenParams` ، تصدیق کرنے والی چابیاں)
جالی اور مددگار ایک دیئے گئے اونچائی پر موثر حیثیت حاصل کرنے کے لئے استعمال کرتے تھے
`crates/iroha_core/src/state.rs:7540` - `7561`۔ رن ٹائم مددگار زیر التوا ہے
ہدف کی اونچائی پر پہنچتے ہی ٹرانزیشن اور بعد میں لاگ ان کی ناکامی
ریبراڈکاسٹ (`crates/iroha_core/src/state.rs:6725` - `6765`)۔

اثاثہ پالیسیاں سرایت کرتی ہیں
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
لہذا گورننس اپ گریڈ کے ذریعے شیڈول کرسکتی ہے
`ScheduleConfidentialPolicyTransition` اور اگر ضرورت ہو تو ان کو منسوخ کریں۔ دیکھو
`crates/iroha_data_model/src/asset/definition.rs:320` اور Torii dto آئینے
(`crates/iroha_torii/src/routing.rs:1539` - `1580`)۔

## گردش کا فلو

1. ** نئے پیرامیٹر بنڈل شائع کریں۔ ** آپریٹرز جمع کروائیں
   `PublishPedersenParams`/`PublishPoseidonParams` ہدایات (CLI
   `iroha app zk params publish ...`) میٹا ڈیٹا کے ساتھ نئے جنریٹر سیٹ اسٹیج کرنے کے لئے ،
   ایکٹیویشن/فرسودگی ونڈوز ، اور اسٹیٹس مارکر۔ پھانسی دینے والا مسترد کرتا ہے
   ڈپلیکیٹ آئی ڈی ، غیر بڑھتی ہوئی ورژن ، یا خراب حیثیت کی منتقلی فی
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499` - `2635` ، اور دی
   رجسٹری ٹیسٹ ناکامی کے طریقوں (`crates/iroha_core/tests/confidential_params_registry.rs:93` - `226`) کا احاطہ کرتے ہیں۔
2. ** رجسٹر/تصدیق کرنے والی کلیدی تازہ کارییں۔
   عزم ، اور سرکٹ/ورژن کی رکاوٹیں اس سے پہلے کہ کوئی کلید داخل ہوسکے
   رجسٹری (`crates/iroha_core/src/smartcontracts/isi/world.rs:2067` - `2137`)۔
   ایک کلید کو اپ ڈیٹ کرنے سے خود بخود پرانے اندراج اور ان لائن بائٹس کو مسح کرتا ہے ،
   جیسا کہ `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1` کے ذریعہ استعمال کیا گیا ہے۔
3. ** شیڈول اثاثہ پالیسی ٹرانزیشن۔
   گورننس نے مطلوبہ کے ساتھ `ScheduleConfidentialPolicyTransition` کو کال کی
   وضع ، منتقلی ونڈو ، اور آڈٹ ہیش۔ پھانسی دینے والا تنازعہ سے انکار کرتا ہے
   بقایا شفاف فراہمی کے ساتھ ٹرانزیشن یا اثاثے۔ ٹیسٹ جیسے
   `crates/iroha_core/tests/confidential_policy_gates.rs:300` - `384` اس کی تصدیق کریں
   اسقاط حمل کی منتقلی `pending_transition` کو صاف کریں ، جبکہ
   `confidential_policy_transition_reaches_shielded_only_on_schedule` at
   لائنز 385–433 کی تصدیق کرتی ہے کہ `ShieldedOnly` پر شیڈول اپ گریڈ پلٹائیں
   موثر اونچائی۔
4. ** پالیسی کی درخواست اور میمپول گارڈ۔
   ہر بلاک (`apply_policy_if_due`) کے آغاز میں ٹرانزیشن اور اخراج
   ٹیلی میٹری اگر کسی منتقلی میں ناکام ہوجاتا ہے تو آپریٹرز دوبارہ شیڈول کرسکتے ہیں۔ داخلہ کے دوران
   میمپول نے لین دین سے انکار کردیا جن کی موثر پالیسی وسط بلاک کو تبدیل کردے گی ،
   منتقلی ونڈو میں ڈٹرمینسٹک شمولیت کو یقینی بنانا
   (`docs/source/confidential_assets.md:60`)۔

## والیٹ اور ایس ڈی کے کی ضروریات- سوئفٹ اور دیگر موبائل ایس ڈی کے فعال پالیسی لانے کے لئے Torii مددگاروں کو بے نقاب کریں
  اس کے علاوہ کوئی زیر التواء منتقلی ، لہذا بٹوے دستخط کرنے سے پہلے صارفین کو متنبہ کرسکتے ہیں۔ دیکھو
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) اور اس سے وابستہ
  `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591` پر ٹیسٹ۔
- CLI اسی میٹا ڈیٹا کو `iroha ledger assets data-policy get` کے ذریعے آئینہ دیتا ہے
  `crates/iroha_cli/src/main.rs:1497` - `1670`) ، آپریٹرز کو آڈٹ کرنے کے قابل بناتا ہے
  پالیسی/پیرامیٹر IDs بغیر کسی اثاثہ کی تعریف میں تیار ہیں
  بلاک اسٹور

## ٹیسٹ اور ٹیلی میٹری کوریج

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288` - `345` اس پالیسی کی تصدیق کرتا ہے
  ٹرانزیشن میٹا ڈیٹا اسنیپ شاٹس میں پھیلتی ہے اور ایک بار لاگو ہونے کے بعد صاف ہوجاتی ہے۔
- `crates/iroha_core/tests/zk_dedup.rs:1` یہ ثابت کرتا ہے کہ `Preverify` کیشے
  جہاں گھومنے والے منظرنامے بھی شامل ہیں ، ڈبل اسپینڈس/ڈبل پروف کو مسترد کرتا ہے
  وعدے مختلف ہیں۔
- `crates/iroha_core/tests/zk_confidential_events.rs` اور
  `zk_shield_transfer_audit.rs` اختتام سے آخر میں شیلڈ → ٹرانسفر → غیر شیلڈ کا احاطہ کرتا ہے
  بہاؤ ، اس بات کو یقینی بنانا کہ آڈٹ کی پگڈنڈی پیرامیٹر کی گردشوں میں زندہ رہتی ہے۔
- `dashboards/grafana/confidential_assets.json` اور
  `docs/source/confidential_assets.md:401` دستاویزات کے عہد نامے &
  تصدیق کنندہ کیشے گیجز جو ہر انشانکن/گردش رن کے ساتھ ہوتے ہیں۔

## رن بک کی ملکیت

- ** ڈیوریل / والیٹ ایس ڈی کے لیڈز: ** ایس ڈی کے کے ٹکڑوں کو برقرار رکھیں + کوئیک اسٹارٹ جو دکھاتے ہیں
  زیر التوا ٹرانزیشن کی سطح کیسے لگائیں اور ٹکسال → ٹرانسفر → انکشاف کریں
  مقامی طور پر ٹیسٹ (`docs/source/project_tracker/confidential_assets_phase_c.md:M3.2` کے تحت ٹریک کیا گیا)۔
- ** پروگرام ایم جی ایم ٹی / خفیہ اثاثوں tl: ** منتقلی کی درخواستوں کو منظور کریں ، رکھیں
  `status.md` آئندہ گردشوں کے ساتھ تازہ کاری کرتا ہے ، اور چھوٹ کو یقینی بناتا ہے (اگر کوئی ہے)
  انشانکن لیجر کے ساتھ ساتھ ریکارڈ کیا گیا۔