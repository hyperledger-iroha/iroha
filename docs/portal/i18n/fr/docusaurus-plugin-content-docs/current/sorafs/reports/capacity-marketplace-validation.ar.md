---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : التحقق من سوق سعة SoraFS
balises : [SF-2c, acceptation, liste de contrôle]
résumé: قائمة تحقق للقبول تغطي انضمام المزودين، تدفقات النزاعات، وتسوية الخزانة التي تضبط جاهزية La clé est SoraFS.
---

# قائمة تحقق التحقق من سوق سعة SoraFS

**نافذة المراجعة:** 2026-03-18 -> 2026-03-24  
** مالكو البرنامج :** Équipe de stockage (`@storage-wg`), Conseil de gouvernance (`@council`), Guilde du Trésor (`@treasury`)  
**النطاق:** مسارات انضمام المزودين، تدفقات تحكيم النزاعات، وعمليات تسوية الخزانة المطلوبة لـ SF-2c GA.

يجب مراجعة قائمة التحقق أدناه قبل تمكين السوق للمشغلين الخارجيين. Il s'agit d'un match de test (tests et rencontres) et d'un match.

## قائمة تحقق القبول

### انضمام المزودين

| الفحص | التحقق | الدليل |
|-------|------------|--------------|
| يقبل registre إعلانات السعة القياسية | Vous pouvez utiliser l'API de l'application `/v2/sorafs/capacity/declare` pour utiliser les métadonnées et les métadonnées. registre العقدة. | `crates/iroha_torii/src/routing.rs:7654` |
| يرفض smart contract et charges utiles غير المتطابقة | Il s'agit d'une source d'informations sur GiB et d'une source d'informations sur GiB. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| يصدر CLI artefacts انضمام قياسية | Le harnais CLI utilise les protocoles Norito/JSON/Base64 pour les allers-retours et les connexions hors ligne. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| يلتقط دليل المشغلين سير قبول الانضمام وحواجز الحوكمة | Il s'agit des valeurs par défaut de la stratégie et des valeurs par défaut. | `../storage-capacity-marketplace.md` |

### تسوية النزاعات

| الفحص | التحقق | الدليل |
|-------|------------|--------------|
| تبقى سجلات النزاع مع digest قياسي للـ payload | Il y a des charges utiles en attente et le grand livre est en attente. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| مولد نزاعات CLI يطابق المخطط القياسي | La CLI utilise Base64/Norito et JSON pour `CapacityDisputeV1` pour les bundles de preuves. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| اختبار replay يثبت حتمية النزاع/العقوبة | la télémétrie est utilisée pour la preuve d'échec et les instantanés, ainsi que pour le grand livre et les barres obliques entre les pairs. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| يوثق runbook مسار التصعيد والإلغاء | يلتقط دليل العمليات سير المجلس ومتطلبات الأدلة وإجراءات restauration. | `../dispute-revocation-runbook.md` |

### تسوية الخزانة| الفحص | التحقق | الدليل |
|-------|------------|--------------|
| تراكم ledger يطابق توقع trempage لمدة 30 يوما | يمتد اختبار tremper عبر خمسة مزودين على 30 نافذة règlement, مع مقارنة إدخالات ledger بالمرجع المتوقع للمدفوعات. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| تسوية صادرات ledger تُسجل ليلا | يقارن `capacity_reconcile.py` توقعات fee ledger بصادرات تحويل XOR المنفذة، ويصدر مقاييس Prometheus, ويضبط موافقة Utilisez Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| لوحات facturation تعرض العقوبات وtélémétrie التراكم | يعرض استيراد Grafana تراكم GiB-hour, عدادات strikes, والضمان المربوط لتمكين الرؤية لدى فريق المناوبة. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| التقرير المنشور يؤرشف منهجية tremper et replay | Vous devez tremper les crochets et les crochets pour les nettoyer. | `./sf2c-capacity-soak.md` |

## ملاحظات التنفيذ

Il s'agit de l'approbation de la signature :

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Vous pouvez utiliser les charges utiles en utilisant JSON/Norito pour `sorafs_manifest_stub capacity {declaration,dispute}`. الناتجة بجانب تذكرة الحوكمة.

## artefacts الموافقة

| Artefact | Chemin | blake2b-256 |
|--------------|------|-------------|
| حزمة موافقة انضمام المزودين | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| حزمة موافقة تسوية النزاعات | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| حزمة موافقة تسوية الخزانة | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Vous devez acheter des artefacts pour créer des objets précieux.

## الموافقات

- Chef d'équipe de stockage — @storage-tl (2026-03-24)  
- Secrétaire du Conseil de gouvernance — @council-sec (2026-03-24)  
- Responsable des opérations de trésorerie — @treasury-ops (2026-03-24)