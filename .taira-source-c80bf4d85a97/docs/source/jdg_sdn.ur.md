---
lang: ur
direction: rtl
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2026-01-03T18:07:58.621058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

٪ JDG-SDN کی تصدیق اور گردش

یہ نوٹ خفیہ ڈیٹا نوڈ (SDN) کی تصدیق کے لئے انفورسمنٹ ماڈل کو اپنی گرفت میں لے رہا ہے
دائرہ اختیار ڈیٹا گارڈین (جے ڈی جی) کے بہاؤ کے ذریعہ استعمال کیا جاتا ہے۔

## عزم کی شکل
- `JdgSdnCommitment` دائرہ کار (`JdgAttestationScope`) ، خفیہ کردہ
  پے لوڈ ہیش ، اور ایس ڈی این پبلک کلید۔ مہروں پر دستخط ہوتے ہیں
  (`SignatureOf<JdgSdnCommitmentSignable>`) ڈومین ٹیگڈ پے لوڈ سے زیادہ
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`۔
- ساختی توثیق (`validate_basic`) نافذ:
  - `version == JDG_SDN_COMMITMENT_VERSION_V1`
  - درست بلاک کی حدود
  - غیر خالی مہریں
  - جب چلتے ہو تو تصدیق کے خلاف دائرہ کار مساوات
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- کٹوتی کی تصدیق کرنے والے (دستخط کنندہ+پے لوڈ ہیش
  انفرادیت) روک تھام/ڈپلیکیٹ وعدوں کو روکنے کے لئے۔

## رجسٹری اور گردش کی پالیسی
- SDN کیز `JdgSdnRegistry` میں براہ راست ، `(Algorithm, public_key_bytes)` کے ذریعہ کی گئی ہے۔
- `JdgSdnKeyRecord` ایکٹیویشن کی اونچائی ، اختیاری ریٹائرمنٹ اونچائی ، ریکارڈ کرتا ہے ،
  اور اختیاری والدین کی کلید۔
- گردش `JdgSdnRotationPolicy` (فی الحال: `dual_publish_blocks` کے ذریعہ چلتی ہے
  اوورلیپ ونڈو)۔ کسی بچے کی کلیدی رجسٹریشن والدین کی ریٹائرمنٹ کو اپ ڈیٹ کرتی ہے
  `child.activation + dual_publish_blocks` ، گارڈریلز کے ساتھ:
  - لاپتہ والدین کو مسترد کردیا جاتا ہے
  - سرگرمیوں میں سختی سے اضافہ ہونا چاہئے
  - فضل کی کھڑکی سے تجاوز کرنے والے اوورلیپس کو مسترد کردیا جاتا ہے
- رجسٹری مدد کرنے والے اسٹیٹس کے لئے انسٹال ریکارڈ (`record` ، `keys`) کی سطح پر
  اور API کی نمائش۔

## توثیق کا بہاؤ
- `JdgAttestation::validate_with_sdn_registry` ساختی لپیٹتا ہے
  تصدیق کی جانچ پڑتال اور SDN نفاذ۔ `JdgSdnPolicy` تھریڈز:
  - `require_commitments`: PII/خفیہ پے لوڈ کے لئے موجودگی کو نافذ کریں
  - `rotation`: والدین کی ریٹائرمنٹ کو اپ ڈیٹ کرتے وقت گریس ونڈو استعمال کیا جاتا ہے
- ہر عزم کی جانچ پڑتال کی جاتی ہے:
  - ساختی جواز + تصدیق- اسکوپ میچ
  - رجسٹرڈ کلیدی موجودگی
  - فعال ونڈو کی تصدیق شدہ بلاک رینج (ریٹائرمنٹ کی حدود پہلے ہی
    دوہری اشاعت شدہ فضل کو شامل کریں)
  - ڈومین ٹیگڈ کمٹمنٹ باڈی پر درست مہر
- مستحکم غلطیاں آپریٹر کے ثبوت کے لئے انڈیکس کی سطح پر ہیں:
  `MissingSdnCommitments` ، `UnknownSdnKey` ، `InactiveSdnKey` ، `InvalidSeal` ،
  یا ساختی `Commitment`/`ScopeMismatch` ناکامی۔

## آپریٹر رن بک
- ** فراہمی: ** `activated_at` کے ساتھ پہلی SDN کلید کو رجسٹر کریں
  پہلا خفیہ بلاک اونچائی۔ جے ڈی جی آپریٹرز کو کلیدی فنگر پرنٹ شائع کریں۔
- ** گھومائیں: ** جانشین کی کلید تیار کریں ، اسے `rotation_parent` کے ساتھ رجسٹر کریں
  موجودہ کلید کی طرف اشارہ کرنا ، اور والدین کی ریٹائرمنٹ کے برابر کی تصدیق کرنا
  `child_activation + dual_publish_blocks`۔ پے لوڈ کے وعدوں پر دوبارہ مہر رکھیں
  اوورلیپ ونڈو کے دوران فعال کلید۔
- **Audit:** expose registry snapshots (`record`, `keys`) via Torii/status
  سطحیں تاکہ آڈیٹر فعال کلید اور ریٹائرمنٹ ونڈوز کی تصدیق کرسکیں۔ انتباہ
  اگر تصدیق شدہ رینج فعال ونڈو کے باہر گرتی ہے۔
- ** بازیافت: ** `UnknownSdnKey` → یقینی بنائیں کہ رجسٹری میں سگ ماہی کی کلید شامل ہے۔
  `InactiveSdnKey` activ ایکٹیویشن ہائٹس کو گھومائیں یا ایڈجسٹ کریں۔ `InvalidSeal` →
  پے لوڈ کو دوبارہ سیل کریں اور تصدیقوں کو ریفریش کریں۔## رن ٹائم مددگار
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) ایک پالیسی + پیکیجز
  رجسٹری اور `validate_with_sdn_registry` کے ذریعے تصدیقوں کی توثیق کرتا ہے۔
- رجسٹریوں کو Norito-encoded `JdgSdnKeyRecord` بنڈل سے بھرا جاسکتا ہے (دیکھیں
  `JdgSdnEnforcer::from_reader`/`from_path`) یا جمع شدہ
  `from_records` ، جو رجسٹریشن کے دوران گردش کے محافظوں کا اطلاق کرتا ہے۔
- آپریٹرز Torii/حیثیت کے ثبوت کے طور پر Norito بنڈل کو برقرار رکھ سکتے ہیں
  سرفیسنگ جبکہ وہی پے لوڈ داخلہ کے ذریعہ استعمال ہونے والے نفاذ کو کھانا کھلاتا ہے اور
  اتفاق رائے کے محافظ۔ ایک واحد عالمی نفاذ کا آغاز اسٹارٹ اپ کے ذریعے کیا جاسکتا ہے
  `init_enforcer_from_path` ، اور `enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  حیثیت/Torii سطحوں کے لئے براہ راست پالیسی + کلیدی ریکارڈ کو بے نقاب کریں۔

## ٹیسٹ
- `crates/iroha_data_model/src/jurisdiction.rs` میں رجعت کی کوریج:
  `sdn_registry_accepts_active_commitment` ، `sdn_registry_rejects_unknown_key` ،
  `sdn_registry_rejects_inactive_key` ، `sdn_registry_rejects_bad_signature` ،
  `sdn_registry_sets_parent_retirement_window` ،
  `sdn_registry_rejects_overlap_beyond_policy` ، موجودہ کے ساتھ ساتھ
  ساختی تصدیق/SDN توثیق کے ٹیسٹ۔