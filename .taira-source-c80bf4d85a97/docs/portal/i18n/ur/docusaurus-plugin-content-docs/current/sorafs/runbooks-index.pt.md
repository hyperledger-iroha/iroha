---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: رن بوکس انڈیکس
عنوان: آپریٹر رن بک انڈیکس
سائڈبار_لیبل: رن بک انڈیکس
تفصیل: ہجرت شدہ SoraFS آپریٹر رن بوکس کے لئے کیننیکل انٹری پوائنٹ۔
---

> ذمہ داروں کے ریکارڈ کی عکاسی کرتا ہے ، جو `docs/source/sorafs/runbooks/` پر واقع ہے۔
> ہر نیا SoraFS آپریشنز گائیڈ کو جیسے ہی شائع ہوتا ہے اسے یہاں لنک کرنا ضروری ہے
> پورٹل بلڈ۔

اس صفحے کا استعمال یہ چیک کرنے کے لئے کریں کہ کون سے رن بکس نے ڈاکٹر ٹری ہجرت کو مکمل کیا ہے
پورٹل کا متبادل۔ ہر اندراج ذمہ داری ، کیننیکل سورس راہ کی فہرست دیتا ہے
اور پورٹل میں کاپی کریں تاکہ جائزہ لینے والے بیٹا پیش نظارہ کے دوران اپنی مطلوبہ گائیڈ میں سیدھے کود سکیں۔

## بیٹا پیش نظارہ میزبان

docops لہر نے پہلے ہی جائزہ لینے والے سے منظور شدہ بیٹا پیش نظارہ میزبان کو فروغ دیا ہے
`SoraFS`۔ جب آپریٹرز یا جائزہ نگاروں کو ہجرت شدہ رن بک کی ہدایت کرتے ہو ،
اس میزبان نام کا حوالہ دیں تاکہ وہ چیکسم سے محفوظ پورٹل اسنیپ شاٹ کا استعمال کرسکیں۔
شائع/رول بیک طریقہ کار میں ہیں
[`devportal/preview-host-exposure`] (../devportal/preview-host-exposure.md)۔

| رن بک | ذمہ دار شخص (زبانیں) | پورٹل پر کاپی | ماخذ |
| --------- | ----------------- | ------------------- | ------- |
| گیٹ وے اور ڈی این ایس کک آف | ٹی ایل نیٹ ورکنگ ، او پی ایس آٹومیشن ، دستاویزات/ڈیوریل | [`sorafs/gateway-dns-runbook`] (./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS آپریشنز پلے بک | دستاویزات/ڈیوریل | [`sorafs/operations-playbook`] (./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| صلاحیت مفاہمت | ٹریژری/سری | [`sorafs/capacity-reconciliation`] (./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| پن لاگ آپریشن | ٹولنگ ڈبلیو جی | [`sorafs/pin-registry-ops`] (./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| نوڈ آپریشنز چیک لسٹ | اسٹوریج ٹیم ، SRE | [`sorafs/node-operations`] (./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| تنازعات اور منسوخیاں رن بک | گورننس کونسل | [`sorafs/dispute-revocation-runbook`] (./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| اسٹیجنگ منشور پلے بک | دستاویزات/ڈیوریل | [`sorafs/staging-manifest-playbook`] (./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| تائکائی اینکر مشاہدہ | میڈیا پلیٹ فارم WG / DA پروگرام / نیٹ ورکنگ TL | [`sorafs/taikai-anchor-runbook`] (./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## توثیق چیک لسٹ

- [x] پورٹل بلڈ اس انڈیکس (سائڈبار میں اندراج) کی طرف اشارہ کرتا ہے۔
- [x] ہر ہجرت شدہ رن بک جائزہ لینے والوں کو برقرار رکھنے کے لئے کیننیکل سورس کے راستے کی فہرست بناتی ہے
  دستاویزات کے جائزوں کے دوران منسلک۔
- [x] جب درج شدہ رن بک ہے تو ڈوکپس پیش نظارہ پائپ لائن بلاکس مل جاتے ہیں
  پورٹل سے باہر نکلنے سے غائب ہے۔

مستقبل کی ہجرت (جیسے نئے افراتفری کے نقوش یا گورننس اپینڈیجز)
مذکورہ ٹیبل میں ایک قطار شامل کریں اور ڈوکپس چیک لسٹ کو شامل کریں جس میں سرایت شدہ ہے
`docs/examples/docs_preview_request_template.md`۔