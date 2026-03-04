---
lang: ur
direction: rtl
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-09T07:05:10.922933+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# جے ڈی جی کی تصدیق: گارڈ ، گردش ، اور برقرار رکھنا

اس نوٹ میں V1 JDG تصدیق گارڈ کی دستاویز کی گئی ہے جو اب `iroha_core` میں جہاز ہے۔

- **Committee manifests:** Norito-encoded `JdgCommitteeManifest` bundles carry per-dataspace rotation
  نظام الاوقات (`committee_id` ، آرڈرڈ ممبران ، دہلیز ، `activation_height` ، `retire_height`)۔
  منشور `JdgCommitteeSchedule::from_path` کے ساتھ بھری ہوئی ہیں اور سختی سے اضافہ کرتے ہیں
  ریٹائرمنٹ/چالو کرنے کے درمیان اختیاری گریس اوورلیپ (`grace_blocks`) کے ساتھ ایکٹیویشن ہائٹس
  کمیٹیاں۔
- ** تصدیق گارڈ: ** `JdgAttestationGuard` ڈیٹا اسپیس بائنڈنگ ، میعاد ختم ہونے ، باسی حدود کو نافذ کرتا ہے ،
  کمیٹی ID/تھریشولڈ مماثل ، دستخط کنندہ کی رکنیت ، معاون دستخطی اسکیمیں ، اور اختیاری
  `JdgSdnEnforcer` کے ذریعے SDN توثیق۔ سائز کیپس ، میکس لیگ ، اور اجازت دینے والی دستخطی اسکیمیں ہیں
  کنسٹرکٹر پیرامیٹرز ؛ `validate(attestation, dataspace, current_height)` فعال کو لوٹاتا ہے
  کمیٹی یا ایک منظم غلطی۔
  - `scheme_id = 1` (`simple_threshold`): فی سگنل دستخط ، اختیاری دستخط کنندہ بٹ میپ۔
  -`scheme_id = 2` (`bls_normal_aggregate`): سنگل پہلے سے حاصل شدہ BLS-معمول کے دستخط پر
    تصدیق ہیش ؛ دستخط کرنے والے بٹ میپ اختیاری ، تصدیق کے تمام دستخط کنندگان کو ڈیفالٹ کرتا ہے۔ bls
    مجموعی توثیق کے لئے منشور میں کمیٹی کے ایک درست پاپ کی ضرورت ہوتی ہے۔ غائب یا
    غلط پوپس تصدیق کو مسترد کرتے ہیں۔
  `governance.jdg_signature_schemes` کے ذریعے اجازت فہرست ترتیب دیں۔
۔
  فی ڈیٹا اسپیس کیپ ، داخل ہونے پر قدیم ترین اندراجات کی کٹائی۔ `for_dataspace` پر کال کریں یا
  `for_dataspace_and_epoch` آڈٹ/ری پلے بنڈل بازیافت کرنے کے لئے۔
- ** ٹیسٹ: ** یونٹ کی کوریج اب درست کمیٹی کا انتخاب ، نامعلوم دستخط کنندہ مسترد ، باسی ورزش کرتی ہے
  تصدیق کو مسترد کرنا ، غیر تعاون یافتہ اسکیم آئی ڈی ، اور برقرار رکھنے کی کٹائی۔ دیکھو
  `crates/iroha_core/src/jurisdiction.rs`۔

گارڈ تشکیل شدہ اجازت فہرست فہرست سے باہر اسکیموں کو مسترد کرتا ہے۔