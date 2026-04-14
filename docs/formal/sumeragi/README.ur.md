<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi رسمی ماڈل (TLA+ / Apalache)

اس ڈائرکٹری میں Sumeragi کمٹ پاتھ سیفٹی اور زندہ دلی کے لیے ایک پابند رسمی ماڈل شامل ہے۔

## دائرہ کار

ماڈل قبضہ کرتا ہے:
- مرحلے کی ترقی (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`)
- ووٹ اور کورم کی حد (`CommitQuorum`, `ViewQuorum`)،
- وزنی اسٹیک کورم (`StakeQuorum`) NPoS طرز کے کمٹ گارڈز کے لیے،
- RBC causality (`Init -> Chunk -> Ready -> Deliver`) ہیڈر/ڈائجسٹ ثبوت کے ساتھ،
- جی ایس ٹی اور ایماندارانہ پیشرفت کے اقدامات پر کمزور منصفانہ مفروضے۔

یہ جان بوجھ کر تار کی شکلوں، دستخطوں، اور نیٹ ورکنگ کی مکمل تفصیلات کا خلاصہ کرتا ہے۔

## فائلیں۔

- `Sumeragi.tla`: پروٹوکول ماڈل اور خصوصیات۔
- `Sumeragi_fast.cfg`: چھوٹا CI-دوستانہ پیرامیٹر سیٹ۔
- `Sumeragi_deep.cfg`: بڑا تناؤ پیرامیٹر سیٹ۔

## خواص

متغیرات:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

عارضی جائیداد:
- `EventuallyCommit` (`[] (gst => <> committed)`)، GST کے بعد کی انصاف کے ساتھ انکوڈ شدہ
  `Next` میں فعال طور پر (ٹائم آؤٹ/فالٹ پریمپشن گارڈز فعال
  ترقی کے اقدامات)۔ یہ Apalache 0.52.x کے ساتھ ماڈل کو چیک کرنے کے قابل رکھتا ہے، جو
  چیک شدہ وقتی خصوصیات کے اندر `WF_` فیئرنس آپریٹرز کو سپورٹ نہیں کرتا ہے۔

## چل رہا ہے۔

ذخیرہ کی جڑ سے:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### دوبارہ پیدا کرنے کے قابل مقامی سیٹ اپ (کوئی Docker درکار نہیں)اس ذخیرے کے ذریعہ استعمال کردہ پن شدہ مقامی اپالاچی ٹول چین کو انسٹال کریں:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

رنر اس انسٹال کا خود بخود پتہ لگاتا ہے:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`۔
تنصیب کے بعد، `ci/check_sumeragi_formal.sh` کو اضافی env vars کے بغیر کام کرنا چاہیے:

```bash
bash ci/check_sumeragi_formal.sh
```

اگر Apalache `PATH` میں نہیں ہے، تو آپ یہ کر سکتے ہیں:

- `APALACHE_BIN` کو قابل عمل راستے پر سیٹ کریں، یا
- Docker فال بیک استعمال کریں (`docker` دستیاب ہونے پر بطور ڈیفالٹ فعال):
  - تصویر: `APALACHE_DOCKER_IMAGE` (پہلے سے طے شدہ `ghcr.io/apalache-mc/apalache:latest`)
  - چلانے والے Docker ڈیمون کی ضرورت ہے۔
  - `APALACHE_ALLOW_DOCKER=0` کے ساتھ فال بیک کو غیر فعال کریں۔

مثالیں:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## نوٹس

- یہ ماڈل قابل عمل زنگ ماڈل ٹیسٹوں کی تکمیل کرتا ہے (تبدیل نہیں کرتا)
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  اور
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`۔
- چیک `.cfg` فائلوں میں مستقل اقدار کے پابند ہیں۔
- PR CI ان چیکوں کو `.github/workflows/pr.yml` کے ذریعے چلاتا ہے۔
  `ci/check_sumeragi_formal.sh`۔