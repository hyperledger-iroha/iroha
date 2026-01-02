---
lang: ur
direction: rtl
source: docs/examples/da_manifest_review_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5c959bd6654d095d2b3785a02e9c2ec162e699ad985b342760b952e38766a66
source_last_modified: "2025-11-12T19:46:29.811940+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/da_manifest_review_template.md کا اردو ترجمہ -->

# ڈیٹا اویلیبلٹی مینی فیسٹ گورننس پیکٹ (ٹیمپلیٹ)

جب پارلیمنٹ پینلز سبسڈیز، takedown، یا ریٹینشن تبدیلیوں کے لئے DA مینی فیسٹس کا
جائزہ لیں (roadmap DA-10) تو یہ ٹیمپلیٹ استعمال کریں۔ Markdown کو گورننس ٹکٹ میں
کاپی کریں، placeholders بھریں، اور نیچے حوالہ دیے گئے signed Norito payloads اور
CI artefacts کے ساتھ مکمل فائل منسلک کریں۔

```markdown
## مینی فیسٹ میٹا ڈیٹا
- مینی فیسٹ نام / ورژن: <string>
- Blob کلاس اور گورننس ٹیگ: <taikai_segment / da.taikai.live>
- BLAKE3 ڈائجسٹ (hex): `<digest>`
- Norito payload ہیش (اختیاری): `<digest>`
- سورس envelope / URL: <https://.../manifest_signatures.json>
- Torii پالیسی snapshot ID: `<unix timestamp or git sha>`

## دستخطوں کی توثیق
- مینی فیسٹ حاصل کرنے کا سورس / storage ٹکٹ: `<hex>`
- توثیق کمانڈ/آؤٹ پٹ: `cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json --manifest-signatures-in=manifest_signatures.json` (لاگ اقتباس منسلک؟)
- `manifest_blake3` جو ٹول نے رپورٹ کیا: `<digest>`
- `chunk_digest_sha3_256` جو ٹول نے رپورٹ کیا: `<digest>`
- کونسل سائنر ملٹی ہیشز:
  - `<did:...>` / `<ed25519 multihash>`
- توثیقی ٹائم اسٹیمپ (UTC): `<2026-02-20T11:04:33Z>`

## ریٹینشن توثیق
| فیلڈ | متوقع (پالیسی) | مشاہدہ شدہ (مینی فیسٹ) | ثبوت |
|------|----------------|-------------------------|------|
| ہاٹ ریٹینشن (سیکنڈز) | <مثال کے طور پر، 86400> | <قدر> | `<torii.da_ingest.replication_policy dump | CI link>` |
| کولڈ ریٹینشن (سیکنڈز) | <مثال کے طور پر، 1209600> | <قدر> |  |
| درکار replicas | <قدر> | <قدر> |  |
| اسٹوریج کلاس | <hot / warm / cold> | <قدر> |  |
| گورننس ٹیگ | <da.taikai.live> | <قدر> |  |

## سیاق
- درخواست کی قسم: <سبسڈی | Takedown | مینی فیسٹ روٹیشن | ہنگامی فریز>
- اصل ٹکٹ / کمپلائنس حوالہ: <لنک یا ID>
- سبسڈی / کرایہ اثر: <متوقع XOR تبدیلی یا "n/a">
- ماڈریشن اپیل لنک (اگر ہو): <case_id یا لنک>

## فیصلہ خلاصہ
- پینل: <انفراسٹرکچر | ماڈریشن | ٹریژری>
- ووٹ گنتی: `<for>/<against>/<abstain>` (کوارم `<threshold>` پورا?)
- ایکٹیویشن / رول بیک ہائٹ یا ونڈو: `<block/slot range>`
- فالو اپ اقدامات:
  - [ ] ٹریژری / کرایہ ops کو اطلاع
  - [ ] شفافیت رپورٹ اپ ڈیٹ کریں (`TransparencyReportV1`)
  - [ ] buffer آڈٹ شیڈول کریں

## اسکیلیشن اور رپورٹنگ
- اسکیلیشن ٹریک: <سبسڈی | Compliance | ہنگامی فریز>
- شفافیت رپورٹ لنک / ID (اگر اپ ڈیٹ ہو): <`TransparencyReportV1` CID>
- Proof-token بنڈل یا ComplianceUpdate حوالہ: <پاتھ یا ٹکٹ ID>
- کرایہ / ریزرو لیجر ڈیلٹا (اگر لاگو ہو): <`ReserveSummaryV1` snapshot link>
- ٹیلیمیٹری snapshot URL(s): <Grafana permalink یا artefact ID>
- پارلیمنٹ منٹس کے لئے نوٹس: <ڈیڈ لائنز / ذمہ داریوں کا خلاصہ>

## منسلکات
- [ ] Signed Norito مینی فیسٹ (`.to`)
- [ ] JSON خلاصہ / CI artefact جو ریٹینشن ویلیوز ثابت کرے
- [ ] Proof token یا compliance پیکٹ (takedown کے لئے)
- [ ] Buffer ٹیلیمیٹری snapshot (`iroha_settlement_buffer_xor`)
```

ہر مکمل پیکٹ کو ووٹ کے Governance DAG اندراج کے تحت محفوظ کریں تاکہ بعد کی ریویوز مکمل
تقریب دوہرائے بغیر مینی فیسٹ ڈائجسٹ کا حوالہ دے سکیں۔

</div>
