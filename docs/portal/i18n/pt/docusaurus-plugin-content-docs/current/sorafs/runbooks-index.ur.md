---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice de runbooks
título: آپریٹر رن بکس کا اشاریہ
sidebar_label: رن بک اشاریہ
description: منتقل شدہ SoraFS آپریٹر رن بکس کے لیے مستند نقطۂ آغاز.
---

> `docs/source/sorafs/runbooks/` میں موجود ذمہ داران کے رجسٹر کی عکاسی کرتا ہے۔
> ہر نئی SoraFS آپریشنز گائیڈ کو پورٹل بلڈ میں شائع ہوتے ہی یہاں لنک کرنا ضروری ہے۔

اس صفحے کا استعمال کریں تاکہ معلوم ہو سکے کہ کون سے رن بکس نے پرانی دستاویزی درخت سے
پورٹل میں منتقلی مکمل کر لی ہے۔ ہر اندراج ملکیت، مستند ماخذ راستہ, اور پورٹل کاپی درج
کرتا ہے تاکہ جائزہ لینے والے بیٹا پری ویو کے دوران مطلوبہ گائیڈ تک سیدھا پہنچ سکیں۔

## بیٹا پری ویو ہوسٹ

DocOps é um aplicativo de gerenciamento de banco de dados que funciona com `https://docs.iroha.tech/`
پر پروموٹ کر دیا ہے۔ جب آپریٹرز یا ریویورز کو کسی منتقل شدہ رن بک کی طرف رہنمائی کریں تو
اسی ہوسٹ نیم کا حوالہ دیں تاکہ وہ checksum سے محفوظ پورٹل اسنیپ شاٹ استعمال کریں۔
پبلشنگ/رول بیک کے طریقۂ کار یہاں ہیں
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| رنبک | Mala | پورٹل کاپی | Mais |
|-------|--------|-----------|------|
| گیٹ وے اور DNS کی ابتدا | TL de rede, automação de operações, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS آپریشنز پلے بُک | Documentos/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| کپیسٹی ریکنسیلی ایشن | Tesouraria / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| پن رجسٹری آپریشنز | GT Ferramentaria | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| نوڈ آپریشنز چیک لسٹ | Equipe de armazenamento, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| تنازع اور منسوخی رن بک | Conselho de Governança | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| اسٹیجنگ مینی فیسٹ پلے بُک | Documentos/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Taikai اینکر آبزرویبیلٹی | GT Plataforma de Mídia / Programa DA / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## تصدیقی چیک لسٹ

- [x] پورٹل بلڈ اس اشاریہ سے لنک کرتا ہے (سائیڈبار اندراج)۔
- [x] ہر منتقل شدہ رن بک مستند ماخذ راستہ درج کرتا ہے تاکہ دستاویزاتی جائزوں کے Joana
  ریویورز ہم آہنگ رہیں۔
- [x] DocOps پری ویو پائپ لائن اس وقت مرج کو روکتی ہے جب فہرست میں شامل رن بک پورٹل آؤٹ پٹ
  سے غائب ہو۔

مستقبل کی منتقلیاں (مثلاً نئے کیاؤس ڈرلز یا گورننس کے ضمیمے) کو اوپر والے جدول میں عرض المزيد
O cartão de crédito do `docs/examples/docs_preview_request_template.md` é o DocOps
چیک لسٹ کو اپ ڈیٹ کرنا چاہیے۔