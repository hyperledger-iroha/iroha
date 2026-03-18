---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: индекс-runbooks
Название: آپریٹر رن بکس کا اشاریہ
Sidebar_label: رن بک اشاریہ
описание: منتقل شدہ SoraFS آپریٹر رن بکس کے لیے مستند نقطۂ آغاز.
---

> `docs/source/sorafs/runbooks/` میں موجود ذمہ داران کے رجسٹر کی کاسی کرتا ہے۔
> ہر نئی SoraFS آپریشنز گائیڈ کو پورٹل بلڈ میں شائع ہوتے ہی یہں لنک کرنا ضروری ہے۔

Если вы хотите, чтобы это произошло, вы можете сделать это в любое время. دستاویزی درخت سے
پورٹل میں منتقلی مکمل کر لی ہے۔ ہر اندراج ملکیت، مستند ماخذ راستہ, اور پورٹل کاپی درج
کرتا ہے تاکہ جائزہ لینے والے بیٹا پری ویو کے دوران مطلوبہ گائیڈ تک سیدھا پہنچ سکیں۔

## بیٹا پری ویو ہوسٹ

DocOps может быть использован в качестве инструмента для работы с документами `https://docs.iroha.tech/`
پروموٹ کر دیا ہے۔ جب آپریٹرز یا ریویورز کو کسی کدہ رن بک کی طرف رہنمائی کریں تو
Чтобы проверить контрольную сумму, необходимо выполнить контрольную сумму. کریں۔
پبلشنگ/رول بیک کے طریقۂ کار یہاں ہیں
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| رن بک | مالکان | پورٹل کاپی | ماخذ |
|-------|--------|-----------|------|
| Использование DNS и DNS | Сетевой TL, автоматизация операций, документация/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS آپریشنز پلے بُک | Документы/Разработчики | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| کپیسٹی ریکنسیلی ایشن | Казначейство / СРИ | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| پن رجسٹری آپریشنز | Инструментальная рабочая группа | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Лучший выбор | Группа хранения, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| تنازع اور منسوخی رن بک | Совет управления | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| اسٹیجنگ مینی فیسٹ پلے بُک | Документы/Разработчики | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Тайкай اینکر آبزرویبیلٹی | Рабочая группа по медиа-платформе / Программа DA / Сеть TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## تصدیقی چیک لسٹ

- [x] پورٹل بلڈ اس اشاریہ سے لنک کرتا ہے (سائیڈبار اندراج)۔
- [x] ہر منتقل شدہ رن بک مستند ماخذ راستہ درج کرتا ہے تاکہ دستاویزاتی جائزوں کے دوران
  ریویورز ہم آہنگ رہیں۔
- [x] DocOps پری ویو پائپ لائن اس وقت مرج کو کتی ہے جب فہرست میں شامل رن بک پورٹل آؤٹ پٹ
  سے غائب ہو۔

Если вы хотите (если вы хотите, чтобы это было так) والے جدول میں ایک
Для этого необходимо использовать `docs/examples/docs_preview_request_template.md` для проверки DocOps.
Как сделать так, чтобы это было удобно