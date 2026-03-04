---
lang: ur
direction: rtl
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2026-01-03T18:08:00.700192+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ڈومین کی توثیق

ڈومین کی توثیق آپریٹرز کو گیٹ ڈومین بنانے اور ایک کمیٹی کے تحت دوبارہ استعمال کرنے کی اجازت دیتی ہے۔ توثیق کا پے لوڈ ایک Norito آبجیکٹ ہے جو چین پر ریکارڈ کیا گیا ہے تاکہ کلائنٹ آڈٹ کرسکیں کہ کس نے تصدیق کی کہ کس ڈومین اور کب۔

## پے لوڈ کی شکل

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: کیننیکل ڈومین شناخت کنندہ
- `committee_id`: انسانی - پڑھنے کے قابل کمیٹی کا لیبل
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: بلاک اونچائیوں کی حدود کی توثیق
- `scope`: اختیاری ڈیٹا اسپیس کے علاوہ ایک اختیاری `[block_start, block_end]` ونڈو (شامل) کہ ** لازمی طور پر ** قبول شدہ بلاک کی اونچائی کا احاطہ کریں
- `signatures`: `body_hash()` پر دستخط (`signatures = []` کے ساتھ توثیق)
- `metadata`: اختیاری Norito میٹا ڈیٹا (پروپوزل IDS ، آڈٹ لنکس ، وغیرہ)

## نفاذ

- توثیق کی ضرورت ہوتی ہے جب Nexus فعال ہو اور `nexus.endorsement.quorum > 0` ، یا جب فی ڈومین پالیسی ضرورت کے مطابق ڈومین کو نشان زد کرتی ہے۔
- توثیق ڈومین/بیان ہیش بائنڈنگ ، ورژن ، بلاک ونڈو ، ڈیٹا اسپیس ممبرشپ ، میعاد ختم/عمر ، اور کمیٹی کورم کو نافذ کرتی ہے۔ دستخط کنندگان کے پاس `Endorsement` کردار کے ساتھ براہ راست اتفاق رائے کی چابیاں ہونی چاہئیں۔ ری پلے کو `body_hash` کے ذریعہ مسترد کردیا جاتا ہے۔
- ڈومین رجسٹریشن سے منسلک توثیق میٹا ڈیٹا کلید `endorsement` استعمال کریں۔ اسی توثیق کا راستہ `SubmitDomainEndorsement` ہدایت کے ذریعہ استعمال کیا جاتا ہے ، جو نیا ڈومین رجسٹر کیے بغیر آڈٹ کے لئے توثیق کو ریکارڈ کرتا ہے۔

## کمیٹیاں اور پالیسیاں

- کمیٹیوں کو - چین (`RegisterDomainCommittee`) پر رجسٹرڈ کیا جاسکتا ہے یا تشکیل ڈیفالٹس (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum` ، ID = `default`) سے اخذ کیا جاسکتا ہے۔
- فی - ڈومین پالیسیاں `SetDomainEndorsementPolicy` (کمیٹی ID ، `max_endorsement_age` ، `required` پرچم) کے ذریعے تشکیل دی گئیں۔ غیر حاضر ہونے پر ، Nexus ڈیفالٹس استعمال ہوتے ہیں۔

## سی ایل آئی مددگار

- ایک توثیق/پر دستخط کریں (آؤٹ پٹ Norito JSON سے STDOUT):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- توثیق جمع کروائیں:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- گورننس کا انتظام کریں:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

توثیق کی ناکامی مستحکم غلطی کے تار (کورم مماثل ، باسی/میعاد ختم ہونے والی توثیق ، ​​اسکوپ مماثلت ، نامعلوم ڈیٹاسپیس ، لاپتہ کمیٹی) کی واپسی۔