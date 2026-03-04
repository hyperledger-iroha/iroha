---
id: runbooks-index
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: آپریٹر رن بکس کا اشاریہ
sidebar_label: رن بک اشاریہ
description: منتقل شدہ SoraFS آپریٹر رن بکس کے لیے مستند نقطۂ آغاز.
---

> `docs/source/sorafs/runbooks/` میں موجود ذمہ داران کے رجسٹر کی عکاسی کرتا ہے۔
> ہر نئی SoraFS آپریشنز گائیڈ کو پورٹل بلڈ میں شائع ہوتے ہی یہاں لنک کرنا ضروری ہے۔

اس صفحے کا استعمال کریں تاکہ معلوم ہو سکے کہ کون سے رن بکس نے پرانی دستاویزی درخت سے
پورٹل میں منتقلی مکمل کر لی ہے۔ ہر اندراج ملکیت، مستند ماخذ راستہ، اور پورٹل کاپی درج
کرتا ہے تاکہ جائزہ لینے والے بیٹا پری ویو کے دوران مطلوبہ گائیڈ تک سیدھا پہنچ سکیں۔

## بیٹا پری ویو ہوسٹ

DocOps کی لہر نے جائزہ لینے والوں کے منظور شدہ بیٹا پری ویو ہوسٹ کو `https://docs.iroha.tech/`
پر پروموٹ کر دیا ہے۔ جب آپریٹرز یا ریویورز کو کسی منتقل شدہ رن بک کی طرف رہنمائی کریں تو
اسی ہوسٹ نیم کا حوالہ دیں تاکہ وہ checksum سے محفوظ پورٹل اسنیپ شاٹ استعمال کریں۔
پبلشنگ/رول بیک کے طریقۂ کار یہاں ہیں
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| رن بک | مالکان | پورٹل کاپی | ماخذ |
|-------|--------|-----------|------|
| گیٹ وے اور DNS کی ابتدا | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS آپریشنز پلے بُک | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| کپیسٹی ریکنسیلی ایشن | Treasury / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| پن رجسٹری آپریشنز | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| نوڈ آپریشنز چیک لسٹ | Storage Team, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| تنازع اور منسوخی رن بک | Governance Council | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| اسٹیجنگ مینی فیسٹ پلے بُک | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Taikai اینکر آبزرویبیلٹی | Media Platform WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## تصدیقی چیک لسٹ

- [x] پورٹل بلڈ اس اشاریہ سے لنک کرتا ہے (سائیڈبار اندراج)۔
- [x] ہر منتقل شدہ رن بک مستند ماخذ راستہ درج کرتا ہے تاکہ دستاویزاتی جائزوں کے دوران
  ریویورز ہم آہنگ رہیں۔
- [x] DocOps پری ویو پائپ لائن اس وقت مرج کو روکتی ہے جب فہرست میں شامل رن بک پورٹل آؤٹ پٹ
  سے غائب ہو۔

مستقبل کی منتقلیاں (مثلاً نئے کیاؤس ڈرلز یا گورننس کے ضمیمے) کو اوپر والے جدول میں ایک
قطار شامل کرنی چاہیے اور `docs/examples/docs_preview_request_template.md` میں موجود DocOps
چیک لسٹ کو اپ ڈیٹ کرنا چاہیے۔
