---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: رن بوکس انڈیکس
عنوان: آپریٹرز کے لئے آپریٹنگ دستورالعمل کا اشاریہ
سائڈبار_لیبل: آپریٹنگ دستورالعمل کا اشاریہ
تفصیل: ہجرت شدہ SoraFS ایکس آپریٹر دستورالعمل کے لئے مجاز انٹری پوائنٹ۔
---

> `docs/source/sorafs/runbooks/` کے تحت مالکان کے ریکارڈ کی عکاسی کرتا ہے۔
> گیٹ وے بلڈ میں شائع ہونے کے بعد SoraFS کے لئے کوئی بھی نیا آپریٹنگ دستی لنک کیا جانا چاہئے۔

کسی بھی آپریٹنگ دستیوں کی جانچ پڑتال کے لئے اس صفحے کا استعمال کریں جس نے میراثی دستاویزات کے درخت سے پورٹل میں منتقلی مکمل کی ہے۔
ہر اندراج میں پورٹل میں ملکیت ، منظور شدہ ماخذ راستہ ، اور ورژن کی فہرست دی جاتی ہے تاکہ جائزہ لینے والے کر سکتے ہیں
ڈیمو پیش نظارہ کے دوران مطلوبہ ڈائرکٹری پر براہ راست تشریف لے جائیں۔

## بیٹا پیش نظارہ میزبان

docops Wave نے اپنے جائزہ لینے والے سے منظور شدہ بیٹا پیش نظارہ میزبان کو اپ گریڈ کیا ہے
`SoraFS`۔ آپریٹرز یا آڈیٹرز کو ہجرت شدہ آپریٹنگ دستی کی ہدایت کرتے وقت ،
اس میزبان نام کی طرف اشارہ کریں تاکہ وہ چیکسم سے محفوظ پورٹل اسنیپ شاٹ کا استعمال کرسکیں۔ اعمال
پوسٹ/واپسی پر واقع ہے
[`devportal/preview-host-exposure`] (../devportal/preview-host-exposure.md)۔

| آپریٹنگ دستی | مالک (زبانیں) | پورٹل ورژن | ماخذ |
| ------------- | ----------- | ------------- | -------- |
| گیٹ وے اور ڈی این ایس لانچ | نیٹ ورکنگ ٹی ایل ، او پی ایس آٹومیشن ، دستاویزات/ڈیورل | [`sorafs/gateway-dns-runbook`] (./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS آپریشنز آپریشنز دستی | دستاویزات/ڈیوریل | [`sorafs/operations-playbook`] (./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| طول و عرض ایڈجسٹمنٹ | ٹریژری / سری | [`sorafs/capacity-reconciliation`] (./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| انسٹالیشن لاگ آپریشن | ٹولنگ ڈبلیو جی | [`sorafs/pin-registry-ops`] (./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| معاہدہ آپریشنز چیک لسٹ | اسٹوریج ٹیم ، SRE | [`sorafs/node-operations`] (./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| تنازعات اور منسوخی آپریٹنگ دستی | گورننس کونسل | [`sorafs/dispute-revocation-runbook`] (./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| منشور اسٹیجنگ آپریٹنگ دستی | دستاویزات/ڈیوریل | [`sorafs/staging-manifest-playbook`] (./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| تائکائی اینکر کی مشاہدہ | میڈیا پلیٹ فارم WG / DA پروگرام / نیٹ ورکنگ TL | [`sorafs/taikai-anchor-runbook`] (./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## چیک لسٹ

- [x] پورٹل بلڈر اس انڈیکس (سائڈبار عنصر) سے لنک کرتا ہے۔
- [x] ہر ہجرت شدہ پلے بوک جائزوں کے دوران جائزہ لینے والوں کو مستقل رکھنے کے لئے منظور شدہ ماخذ کے راستے کا تذکرہ کرتی ہے
  دستاویزات
۔

مستقبل میں ہجرت (جیسے نئی افراتفری کی مشقیں یا گورننس ضمیموں) کو ایک قطار شامل کرنا چاہئے
مذکورہ جدول میں اور شامل دستاویزات چیک لسٹ کو اپ ڈیٹ کریں
`docs/examples/docs_preview_request_template.md`۔