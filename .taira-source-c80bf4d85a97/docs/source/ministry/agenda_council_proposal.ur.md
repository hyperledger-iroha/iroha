---
lang: ur
direction: rtl
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2026-01-03T18:07:57.726224+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ایجنڈا کونسل پروپوزل اسکیما (MINFO-2A)

روڈ میپ حوالہ: ** MINFO-2A-تجویز کی شکل درست کرنے والا۔ **

ایجنڈا کونسل کے ورک فلو نے شہریوں سے سبمڈ بلیک لسٹ اور پالیسی میں تبدیلی کی
گورننس پینل سے پہلے ان کی تجاویز کا جائزہ لیں۔ یہ دستاویز اس کی وضاحت کرتی ہے
کیننیکل پے لوڈ اسکیما ، شواہد کی ضروریات ، اور نقل کا پتہ لگانے کے قواعد
نئے توثیق کار (`cargo xtask ministry-agenda validate`) کے ذریعہ استعمال کیا گیا ہے
پروپوزر JSON گذارشات کو پورٹل پر اپ لوڈ کرنے سے پہلے مقامی طور پر پیش کرسکتے ہیں۔

## پے لوڈ کا جائزہ

ایجنڈا کی تجاویز `AgendaProposalV1` Norito اسکیما استعمال کرتی ہیں
(`iroha_data_model::ministry::AgendaProposalV1`)۔ کھیتوں کو JSON کے طور پر انکوڈ کیا جاتا ہے جب
سی ایل آئی/پورٹل سطحوں کے ذریعے جمع کروانا۔

| فیلڈ | قسم | تقاضے |
| ------- | ------ | ---------------- |
| `version` | `1` (U16) | `AGENDA_PROPOSAL_VERSION_V1` کے برابر ہونا چاہئے۔ |
| `proposal_id` | سٹرنگ (`AC-YYYY-###`) | مستحکم شناخت کنندہ ؛ توثیق کے دوران نافذ کیا گیا۔ |
| `submitted_at_unix_ms` | U64 | یونکس کے عہد کے بعد سے ملی سیکنڈ۔ |
| `language` | سٹرنگ | بی سی پی - 47 ٹیگ (`"en"` ، `"ja-JP"` ، وغیرہ)۔ |
| `action` | ENUM (`add-to-denylist` ، `remove-from-denylist` ، `amend-policy`) | وزارت کی کارروائی کی درخواست کی۔ |
| `summary.title` | سٹرنگ | ≤256 چارس کی سفارش کی گئی۔ |
| `summary.motivation` | سٹرنگ | کارروائی کی ضرورت کیوں ہے۔ |
| `summary.expected_impact` | سٹرنگ | نتائج اگر کارروائی قبول کی گئی ہو۔ |
| `tags[]` | لوئر کیس ڈور | اختیاری ٹریج لیبل۔ اجازت شدہ اقدار: `csam` ، `malware` ، `fraud` ، `harassment` ، `impersonation` ، `policy-escalation` ، `terrorism` ، `spam`۔ |
| `targets[]` | آبجیکٹ | ایک یا زیادہ ہیش خاندانی اندراجات (نیچے ملاحظہ کریں) |
| `evidence[]` | آبجیکٹ | ایک یا زیادہ ثبوت منسلکات (نیچے ملاحظہ کریں) |
| `submitter.name` | سٹرنگ | نام یا تنظیم ڈسپلے کریں۔ |
| `submitter.contact` | سٹرنگ | ای میل ، میٹرکس ہینڈل ، یا فون ؛ عوامی ڈیش بورڈز سے تیار کیا گیا۔ |
| `submitter.organization` | سٹرنگ (اختیاری) | جائزہ لینے والے UI میں مرئی۔ |
| `submitter.pgp_fingerprint` | سٹرنگ (اختیاری) | 40 ہیکس اپر کیس فنگر پرنٹ۔ |
| `duplicates[]` | تار | پہلے پیش کردہ پروپوزل IDs کے اختیاری حوالہ جات۔ |

### ہدف اندراجات (`targets[]`)

ہر ہدف ایک ہیش فیملی ڈائجسٹ کی نمائندگی کرتا ہے جس کا حوالہ تجویز کے ذریعہ کیا جاتا ہے۔

| فیلڈ | تفصیل | توثیق |
| ------- | ------------- | -------------- |
| `label` | جائزہ لینے والے سیاق و سباق کے لئے دوستانہ نام۔ | غیر خالی |
| `hash_family` | ہیش شناخت کنندہ (`blake3-256` ، `sha256` ، وغیرہ)۔ | ASCII خطوط/ہندسے/`-_.` ، ≤48 چارس۔ |
| `hash_hex` | ڈائجسٹ کو لوئر کیس ہیکس میں انکوڈ کیا گیا۔ | ≥16 بائٹس (32 ہیکس چارس) اور لازمی طور پر ہیکس ہونا ضروری ہے۔ |
| `reason` | ڈائجسٹ پر کیوں عمل کرنا چاہئے اس کی مختصر تفصیل۔ | غیر خالی |

توثیق کنندہ اسی کے اندر `hash_family:hash_hex` جوڑے کو ڈپلیکیٹ کرتا ہے
تجویز اور تنازعات کی اطلاع دیتا ہے جب ایک ہی فنگر پرنٹ پہلے ہی موجود ہے
ڈپلیکیٹ رجسٹری (نیچے ملاحظہ کریں)

### ثبوت منسلکات (`evidence[]`)

شواہد اندراجات کی دستاویز جہاں جائزہ لینے والے معاون سیاق و سباق کو لے سکتے ہیں۔| فیلڈ | قسم | نوٹ |
| ------- | ------ | ------- |
| `kind` | ENUM (`url` ، `torii-case` ، `sorafs-cid` ، `attachment`) | ہضم کی ضروریات کا تعین کرتا ہے۔ |
| `uri` | سٹرنگ | HTTP (S) URL ، Torii کیس ID ، یا SoraFS URI۔ |
| `digest_blake3_hex` | سٹرنگ | `sorafs-cid` اور `attachment` قسم کے لئے ضروری ہے۔ دوسروں کے لئے اختیاری۔ |
| `description` | سٹرنگ | جائزہ لینے والوں کے لئے اختیاری فری فارم متن۔ |

### ڈپلیکیٹ رجسٹری

آپریٹرز نقل کو روکنے کے لئے موجودہ فنگر پرنٹس کی رجسٹری کو برقرار رکھ سکتے ہیں
معاملات توثیق کنندہ JSON فائل کو قبول کرتا ہے جس کی شکل میں:

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

جب کسی تجویز کا ہدف کسی اندراج سے مماثل ہوتا ہے تو ، توثیق کرنے والا اس وقت تک ختم نہیں ہوتا جب تک
`--allow-registry-conflicts` کی وضاحت کی گئی ہے (انتباہات اب بھی خارج ہیں)۔
[`cargo xtask ministry-agenda impact`] (impact_assessment_tooling.md) to استعمال کریں
ریفرنڈم کے لئے تیار سمری تیار کریں جو نقل کو عبور کرتے ہیں
رجسٹری اور پالیسی سنیپ شاٹس۔

## سی ایل آئی استعمال

ایک ہی تجویز کو لنٹ کریں اور اسے ڈپلیکیٹ رجسٹری کے خلاف چیک کریں:

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

`--allow-registry-conflicts` کو انتباہات میں ڈپلیکیٹ کامیاب فلموں کو نیچے کرنے کے لئے پاس کریں
تاریخی آڈٹ انجام دینا۔

سی ایل آئی اسی Norito اسکیما اور توثیق کے مددگاروں پر انحصار کرتا ہے
`iroha_data_model` ، لہذا SDKS/پورٹل `AgendaProposalV1::validate` کو دوبارہ استعمال کرسکتے ہیں
مستقل سلوک کا طریقہ۔

## ترتیب CLI (MINFO-2B)

روڈ میپ کا حوالہ: ** منفو -2 بی-ملٹی سلاٹ ترتیب اور آڈٹ لاگ۔ **

ایجنڈا کونسل روسٹر اب عین مطابق ترتیب کے ذریعہ منظم کیا جاتا ہے لہذا شہریوں
آزادانہ طور پر ہر قرعہ اندازی کا آڈٹ کر سکتے ہیں۔ نیا کمانڈ استعمال کریں:

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` - JSON فائل ہر اہل ممبر کو بیان کرتی ہے:

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  مثال فائل میں زندہ ہے
  `docs/examples/ministry/agenda_council_roster.json`۔ اختیاری فیلڈز (کردار ،
  تنظیم ، رابطہ ، میٹا ڈیٹا) مرکل کے پتے میں پکڑے جاتے ہیں لہذا آڈیٹر
  ڈرا کو کھلانے والے روسٹر کو ثابت کرسکتے ہیں۔

- `--slots` - کونسل کی نشستوں کی تعداد بھرنے کے لئے۔
- `--seed`- 32-بائٹ بلیک 3 سیڈ (64 لوئر کیس ہیکس حروف)
  قرعہ اندازی کے لئے گورننس منٹ۔
- `--out` - اختیاری آؤٹ پٹ راستہ۔ جب اسے چھوڑ دیا جاتا ہے تو ، JSON کا خلاصہ پرنٹ ہوتا ہے
  stdout.

### آؤٹ پٹ کا خلاصہ

کمانڈ ایک `SortitionSummary` JSON BLOB خارج کرتا ہے۔ نمونہ آؤٹ پٹ پر محفوظ ہے
`docs/examples/ministry/agenda_sortition_summary_example.json`۔ کلیدی فیلڈز:

| فیلڈ | تفصیل |
| ------- | --------------- |
| `algorithm` | ترتیب لیبل (`agenda-sortition-blake3-v1`)۔ |
| `roster_digest` | بلیک 3 + SHA-256 روسٹر فائل کے ہضم (ایک ہی ممبر کی فہرست میں آڈٹ کی تصدیق کے لئے استعمال کیا جاتا ہے)۔ |
| `seed_hex` / `slots` | سی ایل آئی ان پٹ کی بازگشت کریں تاکہ آڈیٹر ڈرا کو دوبارہ پیش کرسکیں۔ |
| `merkle_root_hex` | روسٹر مرکل ٹری کی جڑ (`hash_node`/`hash_leaf` `xtask/src/ministry_agenda.rs` میں مددگار)۔ |
| `selected[]` | ہر سلاٹ کے لئے اندراجات ، بشمول کیننیکل ممبر میٹا ڈیٹا ، اہل انڈیکس ، اصل روسٹر انڈیکس ، ڈٹرمینسٹک ڈرا اینٹروپی ، پتی ہیش ، اور مرکل پروف بہن بھائی۔ |

### قرعہ اندازی کی تصدیق کرنا1. `roster_path` کے ذریعہ حوالہ کردہ روسٹر کو بازیافت کریں اور اس کے بلیک 3/SHA-256 کی تصدیق کریں
   ہضم خلاصہ سے ملتے ہیں۔
2. اسی بیج/سلاٹ/روسٹر کے ساتھ سی ایل آئی کو دوبارہ چلائیں۔ نتیجہ `selected[].member_id`
   آرڈر شائع شدہ خلاصہ سے مماثل ہونا چاہئے۔
3. کسی مخصوص ممبر کے لئے ، سیریلائزڈ ممبر JSON کا استعمال کرتے ہوئے مرکل کے پتے کی گنتی کریں
   (`norito::json::to_vec(&sortition_member)`) اور ہر پروف ہیش میں فولڈ۔ فائنل
   ڈائجسٹ کو `merkle_root_hex` کے برابر ہونا چاہئے۔ مثال کے طور پر سمری شوز میں مددگار
   `eligible_index` ، `leaf_hash_hex` ، اور `merkle_proof[]` کو یکجا کریں۔

یہ نوادرات تصدیق شدہ بے ترتیب پن کے لئے MINFO-2B کی ضرورت کو پورا کرتے ہیں ،
K-OF-M کا انتخاب ، اور جب تک آن چین API وائرڈ نہیں ہوتا ہے اس وقت تک ضمیمہ آڈٹ لاگ ان ہوتا ہے۔

## توثیق کی غلطی کا حوالہ

`AgendaProposalV1::validate` `AgendaProposalValidationError` مختلف حالتوں کا اخراج کرتا ہے
جب بھی پے لوڈ لیٹنگ میں ناکام ہوجاتا ہے۔ نیچے دیئے گئے جدول میں سب سے عام کا خلاصہ پیش کیا گیا ہے
غلطیاں تو پورٹل جائزہ لینے والے CLI آؤٹ پٹ کو قابل عمل رہنمائی میں ترجمہ کرسکتے ہیں۔| غلطی | مطلب | علاج |
| ------- | --------- | --------------- |
| `UnsupportedVersion { expected, found }` | پے لوڈ `version` ویلیویٹر کے تعاون یافتہ اسکیما سے مختلف ہے۔ | تازہ ترین اسکیما بنڈل کا استعمال کرتے ہوئے JSON کو دوبارہ تخلیق کریں تاکہ ورژن `expected` سے مماثل ہے۔ |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` خالی ہے یا نہیں `AC-YYYY-###` فارم میں۔ | دوبارہ سبمیٹنگ سے پہلے دستاویزی شکل کے بعد ایک انوکھا شناخت کنندہ آباد کریں۔ |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` صفر یا لاپتہ ہے۔ | یونکس ملی سیکنڈ میں جمع کرانے کے ٹائم اسٹیمپ کو ریکارڈ کریں۔ |
| `InvalidLanguageTag { value }` | `language` ایک درست BCP - 47 ٹیگ نہیں ہے۔ | ایک معیاری ٹیگ جیسے `en` ، `ja-JP` ، یا BCP - 47 کے ذریعہ تسلیم شدہ دوسرا مقام استعمال کریں۔ |
| `MissingSummaryField { field }` | `summary.title` میں سے ایک ، `.motivation` ، یا `.expected_impact` خالی ہے۔ | اشارہ کردہ سمری فیلڈ کے لئے غیر خالی متن فراہم کریں۔ |
| `MissingSubmitterField { field }` | `submitter.name` یا `submitter.contact` غائب ہے۔ | لاپتہ جمع کرانے والے میٹا ڈیٹا کی فراہمی کریں تاکہ جائزہ لینے والے تجویز کنندہ سے رابطہ کرسکیں۔ |
| `InvalidTag { value }` | `tags[]` اندراج اجازت نہیں ہے۔ | دستاویزی اقدار (`csam` ، `malware` ، وغیرہ) میں سے کسی ایک پر ٹیگ کو ہٹا دیں یا ان کا نام تبدیل کریں۔ |
| `MissingTargets` | `targets[]` سرنی خالی ہے۔ | کم از کم ایک ہدف ہیش فیملی انٹری فراہم کریں۔ |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` | ہدف اندراج `label` یا `reason` فیلڈز سے محروم ہے۔ | دوبارہ شروع کرنے سے پہلے اشاریہ اندراج کے لئے مطلوبہ فیلڈ کو پُر کریں۔ |
| `InvalidHashFamily { index, value }` | غیر تعاون یافتہ `hash_family` لیبل۔ | ہیش فیملی ناموں کو ASCII الفانومرکس پلس `-_` پر محدود رکھیں۔ |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` | ڈائجسٹ درست نہیں ہے یا 16 بائٹس سے کم ہے۔ | انڈیکسڈ ہدف کے لئے ایک لوئر کیس ہیکس ڈائجسٹ (≥32 ہیکس چارس) فراہم کریں۔ |
| `DuplicateTarget { index, fingerprint }` | ہدف ڈائجسٹ پہلے اندراج یا رجسٹری فنگر پرنٹ کی نقل تیار کرتا ہے۔ | نقول کو ہٹا دیں یا معاون شواہد کو ایک ہی ہدف میں ضم کریں۔ |
| `MissingEvidence` | کوئی ثبوت منسلک نہیں کیا گیا تھا۔ | کم از کم ایک ثبوت ریکارڈ جو پنروتپادن کے مواد سے منسلک ہے۔ |
| `MissingEvidenceUri { index }` | ثبوت اندراج `uri` فیلڈ سے محروم ہے۔ | اشاریہ شدہ ثبوت کے اندراج کے ل featetetable URI یا کیس کی شناخت کنندہ فراہم کریں۔ |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` | ثبوت اندراج جس میں ڈائجسٹ کی ضرورت ہوتی ہے (SoraFS CID یا منسلک) غائب ہے یا `digest_blake3_hex` غلط ہے۔ | اشاریہ اندراج کے لئے 64-کردار لوئر کیس بلیک 3 ڈائجسٹ فراہم کریں۔ |

## مثالیں

- `docs/examples/ministry/agenda_proposal_example.json` - کیننیکل ،
  دو شواہد منسلکات کے ساتھ لنٹ کلین پروپوزل پے لوڈ۔
- `docs/examples/ministry/agenda_duplicate_registry.json` - اسٹارٹر رجسٹری
  ایک ہی بلیک 3 فنگر پرنٹ اور عقلی اصول پر مشتمل ہے۔

جب پورٹل ٹولنگ کو مربوط کرتے ہو یا CI لکھتے ہو تو ان فائلوں کو بطور ٹیمپلیٹس دوبارہ استعمال کریں
خودکار گذارشات کے لئے چیک۔