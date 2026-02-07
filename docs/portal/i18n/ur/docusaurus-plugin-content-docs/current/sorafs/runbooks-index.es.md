---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: رن بوکس انڈیکس
عنوان: آپریٹر رن بک انڈیکس
سائڈبار_لیبل: رن بک انڈیکس
تفصیل: منتقلی SoraFS آپریٹر رن بوکس کے لئے کیننیکل انٹری پوائنٹ۔
---

> ذمہ دار افراد کی رجسٹری کی عکاسی کرتا ہے جو `docs/source/sorafs/runbooks/` میں رہتا ہے۔
> ہر نیا SoraFS ٹریڈنگ گائیڈ پر شائع ہونے کے بعد یہاں لنک کیا جانا چاہئے
> پورٹل کی تعمیر۔

اس صفحے کو اس بات کی تصدیق کے لئے استعمال کریں کہ کون سے رن بوکس نے ہجرت کو مکمل کیا ہے
دستاویزات کا درخت پورٹل میں وراثت میں ملا ہے۔ ہر اندراج میں ملکیت کی فہرست دی جاتی ہے ،
کیننیکل ماخذ کا راستہ اور اس کو پورٹل میں کاپی کریں تاکہ جائزہ لینے والے کر سکتے ہیں
بیٹا پیش نظارہ کے دوران براہ راست مطلوبہ گائیڈ پر جائیں۔

## بیٹا پیش نظارہ میزبان

دستاویزات کے اضافے نے پہلے ہی جائزہ لینے والے سے منظور شدہ بیٹا پیش نظارہ میزبان کو فروغ دیا ہے
`SoraFS`۔ جب آپریٹرز یا جائزہ نگاروں کو ہجرت شدہ رن بک کی ہدایت کرتے ہو ،
اس میزبان نام کا حوالہ دیں تاکہ وہ چیکسم سے محفوظ پورٹل اسنیپ شاٹ کا استعمال کریں۔
شائع/رول بیک کے طریقہ کار میں رہتے ہیں
[`devportal/preview-host-exposure`] (../devportal/preview-host-exposure.md)۔

| رن بک | مالک (زبانیں) | پورٹل پر کاپی | ماخذ |
| --------- | ---------------- |
| گیٹ وے اور ڈی این ایس بوٹ | نیٹ ورکنگ ٹی ایل ، او پی ایس آٹومیشن ، دستاویزات/ڈیورل | [`sorafs/gateway-dns-runbook`] (./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS ٹریڈنگ پلے بک | دستاویزات/ڈیوریل | [`sorafs/operations-playbook`] (./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| صلاحیت مفاہمت | ٹریژری/سری | [`sorafs/capacity-reconciliation`] (./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| پن رجسٹری آپریشنز | ٹولنگ ڈبلیو جی | [`sorafs/pin-registry-ops`] (./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| نوڈ آپریشنز چیک لسٹ | اسٹوریج ٹیم ، SRE | [`sorafs/node-operations`] (./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| تنازعات اور منسوخیاں رن بک | گورننس کونسل | [`sorafs/dispute-revocation-runbook`] (./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| اسٹیجنگ میں منشور کی پلے بک | دستاویزات/ڈیوریل | [`sorafs/staging-manifest-playbook`] (./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| تائکائی اینکر مشاہدہ | میڈیا پلیٹ فارم WG / DA پروگرام / نیٹ ورکنگ TL | [`sorafs/taikai-anchor-runbook`] (./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## چیک لسٹ

- [x] اس انڈیکس (سائڈبار انٹری) کے پورٹل بلڈ لنکس۔
- [x] ہر ہجرت شدہ رن بک جائزہ لینے والوں کو برقرار رکھنے کے لئے کیننیکل سورس کے راستے کی فہرست بناتی ہے
  دستاویزات کے جائزوں کے دوران منسلک۔
- [x] جب رن بک غائب ہو تو ڈوکپس پیش نظارہ پائپ لائن بلاکس مل جاتے ہیں
  پورٹل سے باہر نکلنے میں درج ہے۔

مستقبل میں ہجرت (جیسے نئی افراتفری کی مشقیں یا گورننس ایڈینڈمز)
انہیں مندرجہ بالا ٹیبل میں ایک قطار شامل کرنا ہوگی اور اس میں بنی دستاویزات چیک لسٹ کو اپ ڈیٹ کرنا ہوگا
`docs/examples/docs_preview_request_template.md`۔