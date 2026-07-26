---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: رن بوکس انڈیکس
عنوان: آپریٹر رن بکس کا انڈیکس
سائڈبار_لیبل: رن بک انڈیکس
تفصیل: منتقلی SoraFS آپریٹر رن بوکس کے لئے کیننیکل انٹری پوائنٹ۔
---

> `docs/source/sorafs/runbooks/` میں پائے جانے والے مینیجر رجسٹر کی عکاسی کرتا ہے۔
> ہر نئی آپریٹنگ گائیڈ SoraFS کو جیسے ہی شائع ہوتا ہے اسے یہاں لنک کرنا چاہئے
> پورٹل بلڈ۔

اس صفحے کو یہ چیک کرنے کے لئے استعمال کریں کہ کون سے رن بوکس نے درختوں کی منتقلی مکمل کی ہے
پورٹل پر وراثت میں موجود دستاویزات کی۔ ہر اندراج ذمہ داری ، ماخذ کے راستے کی نشاندہی کرتا ہے
کیننیکل اور پورٹل کاپی تاکہ جائزہ لینے والے براہ راست گائیڈ تک رسائی حاصل کرسکیں
بیٹا پیش نظارہ کے دوران مطلوبہ۔

## بیٹا پیش نظارہ میزبان

دستاویزات کی لہر نے اب جائزہ لینے والے سے منظور شدہ بیٹا پیش نظارہ میزبان کو فروغ دیا ہے
`SoraFS`۔ آپریٹرز یا جائزہ لینے والوں کو a کی ہدایت کرتے وقت
ہجرت شدہ رن بک ، اس میزبان نام کا حوالہ دیں تاکہ وہ پورٹل اسنیپ شاٹ دیکھیں
چیکسم کے ذریعہ محفوظ ہے۔ اشاعت/رول بیک طریقہ کار میں پایا جاسکتا ہے
[`devportal/preview-host-exposure`] (../devportal/preview-host-exposure.md)۔

| رن بک | منیجر (زبانیں) | پورٹل کاپی | ماخذ |
| -------- | ---------------- | -------- | -------- |
| گیٹ وے اور ڈی این ایس لانچ | نیٹ ورکنگ ٹی ایل ، او پی ایس آٹومیشن ، دستاویزات/ڈیورل | [`sorafs/gateway-dns-runbook`] (./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| آپریٹنگ پلے بوک SoraFS | دستاویزات/ڈیوریل | [`sorafs/operations-playbook`] (./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| صلاحیت مفاہمت | ٹریژری / سری | [`sorafs/capacity-reconciliation`] (./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| پن رجسٹری آپس | ٹولنگ ڈبلیو جی | [`sorafs/pin-registry-ops`] (./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| نوڈ استحصال چیک لسٹ | اسٹوریج ٹیم ، SRE | [`sorafs/node-operations`] (./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| قانونی چارہ جوئی اور منسوخی رن بک | گورننس کونسل | [`sorafs/dispute-revocation-runbook`] (./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| اسٹیجنگ منشور پلے بک | دستاویزات/ڈیوریل | [`sorafs/staging-manifest-playbook`] (./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| تائکائی اینکر کی مشاہدہ | میڈیا پلیٹ فارم WG / DA پروگرام / نیٹ ورکنگ TL | [`sorafs/taikai-anchor-runbook`] (./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## چیک لسٹ

- [x] پورٹل بلڈ لنکس اس انڈیکس (سائڈبار انٹری) کے لنکس۔
- [x] ہر ہجرت شدہ رن بک جائزہ لینے والوں کو برقرار رکھنے کے لئے کیننیکل سورس کے راستے کی فہرست بناتی ہے
  دستاویزات کے جائزوں کے دوران منسلک۔
- [x] جب درج شدہ رن بوک سے لاپتہ ہو تو ڈوکوپس پیش نظارہ پائپ لائن بلاکس مل جاتی ہے
  گیٹ سے باہر نکلیں۔

مستقبل میں ہجرت (جیسے نئی افراتفری کی مشقیں یا گورننس ملحقہ)
مندرجہ بالا جدول میں ایک قطار شامل کرنا ہوگی اور انضمام شدہ دستاویزات چیک لسٹ کو اپ ڈیٹ کریں
`docs/examples/docs_preview_request_template.md`۔