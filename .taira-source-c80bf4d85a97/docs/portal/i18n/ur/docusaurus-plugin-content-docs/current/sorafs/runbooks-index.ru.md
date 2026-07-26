---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: رن بوکس انڈیکس
عنوان: آپریٹر رین بکس کا انڈیکس
سائڈبار_لیبل: رن بک انڈیکس
تفصیل: ہجرت شدہ آپریٹر رن بکس SoraFS کے لئے کیننیکل انٹری پوائنٹ۔
---

> مالک کی رجسٹری کی عکاسی کرتی ہے ، جو `docs/source/sorafs/runbooks/` میں واقع ہے۔
> ہر نیا آپریشن دستی SoraFS میں اشاعت کے بعد یہاں منسلک ہونا چاہئے
> پورٹل اسمبلی۔

اس صفحے کو یہ چیک کرنے کے لئے استعمال کریں کہ کون سی رن بکس نے اپنی میراثی ہجرت کو مکمل کیا ہے
پورٹل میں دستاویزات کا درخت۔ ہر اندراج میں مالکان ، کیننیکل ماخذ کا راستہ ہوتا ہے
اور پورٹل میں ایک کاپی تاکہ جائزہ لینے والے بیٹا پیش نظارہ کے دوران اپنی ضرورت کے رہنما میں سیدھے کود سکیں۔

## میزبان بیٹا پیش نظارہ

دستاویزات کی لہر نے پہلے ہی جائزہ لینے والے سے منظور شدہ بیٹا پیش نظارہ میزبان کو `https://docs.iroha.tech/` میں فروغ دیا ہے۔
آپریٹرز یا جائزہ نگاروں کو ہجرت شدہ رن بک کی ہدایت کرتے وقت ، اس نام کا استعمال کریں
میزبان تاکہ وہ چیکسم کے ذریعہ محفوظ پورٹل کے اسنیپ شاٹ کے ساتھ کام کریں۔ طریقہ کار
اشاعت/رول بیکس میں ہیں
[`devportal/preview-host-exposure`] (../devportal/preview-host-exposure.md)۔

| رن بک | مالکان | پورٹل میں کاپی | ماخذ |
| -------- | ------------ | ----------------- | ------------ |
| گیٹ وے اور ڈی این ایس لانچنگ | نیٹ ورکنگ ٹی ایل ، او پی ایس آٹومیشن ، دستاویزات/ڈیورل | [`sorafs/gateway-dns-runbook`] (./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| آپریشنز پلے بوک SoraFS | دستاویزات/ڈیوریل | [`sorafs/operations-playbook`] (./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| صلاحیت کی توثیق | ٹریژری/سری | [`sorafs/capacity-reconciliation`] (./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| پن رجسٹری آپریشنز | ٹولنگ ڈبلیو جی | [`sorafs/pin-registry-ops`] (./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| نوڈ آپریشنز چیک لسٹ | اسٹوریج ٹیم ، SRE | [`sorafs/node-operations`] (./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| تنازعات اور جائزوں کی رن بک | گورننس کونسل | [`sorafs/dispute-revocation-runbook`] (./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| اسٹیجنگ منشور کی پلے بک | دستاویزات/ڈیوریل | [`sorafs/staging-manifest-playbook`] (./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| تائکائی اینکر مشاہدہ | میڈیا پلیٹ فارم WG / DA پروگرام / نیٹ ورکنگ TL | [`sorafs/taikai-anchor-runbook`] (./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## چیک لسٹ

- [x] پورٹل اسمبلی اس انڈیکس (سائڈبار عنصر) کا حوالہ دیتا ہے۔
- [x] ہر ہجرت شدہ رن بک رکھنے کے لئے کیننیکل سورس کا راستہ بتاتا ہے
  جائزہ لینے والوں نے دستاویزات کے جائزے کے دوران اتفاق کیا۔
- [x] درج ہونے پر dopops پیش نظارہ پائپ لائن بلاکس ضم ہوجاتے ہیں
  رن بوک پورٹل آؤٹ پٹ سے غائب ہے۔

مستقبل میں ہجرت (مثال کے طور پر ، نئی افراتفری کی مشقیں یا انتظامی ایپلی کیشنز) کو شامل کرنا چاہئے
مذکورہ جدول میں لائن کریں اور میں تعمیر کردہ دستاویزات چیک لسٹ کو اپ ڈیٹ کریں
`docs/examples/docs_preview_request_template.md`۔