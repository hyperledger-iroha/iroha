---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ffbb145e1e0aa9dc71bdb6896c4f8be69eb6226194c5c165905af1ac243cc9
source_last_modified: "2025-11-20T07:34:15.652032+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: التحقق من سوق سعة SoraFS
tags: [SF-2c, acceptance, checklist]
summary: قائمة تحقق للقبول تغطي انضمام المزودين، تدفقات النزاعات، وتسوية الخزانة التي تضبط جاهزية الإطلاق العام لسوق سعة SoraFS.
---

# قائمة تحقق التحقق من سوق سعة SoraFS

**نافذة المراجعة:** 2026-03-18 -> 2026-03-24  
**مالكو البرنامج:** Storage Team (`@storage-wg`)، Governance Council (`@council`)، Treasury Guild (`@treasury`)  
**النطاق:** مسارات انضمام المزودين، تدفقات تحكيم النزاعات، وعمليات تسوية الخزانة المطلوبة لـ SF-2c GA.

يجب مراجعة قائمة التحقق أدناه قبل تمكين السوق للمشغلين الخارجيين. كل صف يربط إلى دليل حتمي (tests أو fixtures أو توثيق) يمكن للمدققين إعادة تشغيله.

## قائمة تحقق القبول

### انضمام المزودين

| الفحص | التحقق | الدليل |
|-------|------------|----------|
| يقبل registry إعلانات السعة القياسية | يقوم اختبار تكاملي بتشغيل `/v1/sorafs/capacity/declare` عبر app API، مع التحقق من معالجة التواقيع، التقاط metadata، وتسليمها إلى registry العقدة. | `crates/iroha_torii/src/routing.rs:7654` |
| يرفض smart contract الـ payloads غير المتطابقة | يضمن اختبار وحدات أن معرفات المزود وحقول GiB الملتزم بها تطابق الإعلان الموقع قبل الحفظ. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| يصدر CLI artefacts انضمام قياسية | يقوم CLI harness بكتابة مخرجات Norito/JSON/Base64 حتمية ويتحقق من round-trips حتى يتمكن المشغلون من إعداد الإعلانات offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| يلتقط دليل المشغلين سير قبول الانضمام وحواجز الحوكمة | توثيق يعدد مخطط الإعلان، policy defaults، وخطوات المراجعة للمجلس. | `../storage-capacity-marketplace.md` |

### تسوية النزاعات

| الفحص | التحقق | الدليل |
|-------|------------|----------|
| تبقى سجلات النزاع مع digest قياسي للـ payload | يسجل اختبار وحدات نزاعا، ويفك payload المخزن، ويؤكد حالة pending لضمان حتمية ledger. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| مولد نزاعات CLI يطابق المخطط القياسي | يغطي اختبار CLI مخرجات Base64/Norito وملخصات JSON لـ `CapacityDisputeV1`، بما يضمن أن evidence bundles تُهش بشكل حتمي. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| اختبار replay يثبت حتمية النزاع/العقوبة | telemetry الخاصة بـ proof-failure التي تُعاد مرتين تنتج snapshots متطابقة للـ ledger والائتمان والنزاع، ليبقى slashes حتميا بين peers. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| يوثق runbook مسار التصعيد والإلغاء | يلتقط دليل العمليات سير المجلس ومتطلبات الأدلة وإجراءات rollback. | `../dispute-revocation-runbook.md` |

### تسوية الخزانة

| الفحص | التحقق | الدليل |
|-------|------------|----------|
| تراكم ledger يطابق توقع soak لمدة 30 يوما | يمتد اختبار soak عبر خمسة مزودين على 30 نافذة settlement، مع مقارنة إدخالات ledger بالمرجع المتوقع للمدفوعات. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| تسوية صادرات ledger تُسجل ليلا | يقارن `capacity_reconcile.py` توقعات fee ledger بصادرات تحويل XOR المنفذة، ويصدر مقاييس Prometheus، ويضبط موافقة الخزانة عبر Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| لوحات billing تعرض العقوبات وtelemetry التراكم | يعرض استيراد Grafana تراكم GiB-hour، عدادات strikes، والضمان المربوط لتمكين الرؤية لدى فريق المناوبة. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| التقرير المنشور يؤرشف منهجية soak وأوامر replay | يوضح التقرير نطاق soak وأوامر التنفيذ وhooks للرصد من اجل المدققين. | `./sf2c-capacity-soak.md` |

## ملاحظات التنفيذ

أعد تشغيل حزمة التحقق قبل sign-off:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

يجب على المشغلين إعادة توليد payloads طلبات الانضمام/النزاع عبر `sorafs_manifest_stub capacity {declaration,dispute}` وأرشفة بايتات JSON/Norito الناتجة بجانب تذكرة الحوكمة.

## artefacts الموافقة

| Artefact | Path | blake2b-256 |
|----------|------|-------------|
| حزمة موافقة انضمام المزودين | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| حزمة موافقة تسوية النزاعات | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| حزمة موافقة تسوية الخزانة | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

احتفظ بالنسخ الموقعة من هذه artefacts مع حزمة الإصدار واربطها في سجل تغييرات الحوكمة.

## الموافقات

- Storage Team Lead — @storage-tl (2026-03-24)  
- Governance Council Secretary — @council-sec (2026-03-24)  
- Treasury Operations Lead — @treasury-ops (2026-03-24)
