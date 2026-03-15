---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: التحقق من سوق سعة SoraFS
Etiquetas: [SF-2c, aceptación, lista de verificación]
resumen: قائمة تحقق للقبول تغطي انضمام المزودين، تدفقات النزاعات، وتسوية الخزانة التي تضبط جاهزية الإطلاق Lea aquí SoraFS.
---

# قائمة تحقق التحقق من سوق سعة SoraFS

**نافذة المراجعة:** 2026-03-18 -> 2026-03-24  
**مالكو البرنامج:** Equipo de almacenamiento (`@storage-wg`), Consejo de gobierno (`@council`), Gremio de tesorería (`@treasury`)  
**النطاق:** Los dispositivos de seguridad están instalados en el SF-2c GA.

يجب مراجعة قائمة التحقق أدناه قبل تمكين السوق للمشغلين الخارجيين. كل صف يربط إلى دليل حتمي (pruebas أو accesorios أو توثيق) يمكن للمدققين إعادة تشغيله.

## قائمة تحقق القبول

### انضمام المزودين| الفحص | التحقق | الدليل |
|-------|------------|----------|
| registro يقبل إعلانات السعة القياسية | Esta es la API de la aplicación `/v2/sorafs/capacity/declare`, que contiene metadatos y registros del registro. العقدة. | `crates/iroha_torii/src/routing.rs:7654` |
| يرفض contrato inteligente الـ cargas útiles غير المتطابقة | يضمن اختبار وحدات أن معرفات المزود وحقول GiB الملتزم بها تطابق الإعلان الموقع قبل الحفظ. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| يصدر CLI artefactos انضمام قياسية | El arnés CLI utiliza Norito/JSON/Base64 y realiza viajes de ida y vuelta sin conexión. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Artículos de tocador y artículos para el hogar | توثيق يعدد مخطط الإعلان، política predeterminada, وخطوات المراجعة للمجلس. | `../storage-capacity-marketplace.md` |

### تسوية النزاعات| الفحص | التحقق | الدليل |
|-------|------------|----------|
| تبقى سجلات النزاع مع resumen قياسي للـ carga útil | يسجل اختبار وحدات نزاعا، ويفك carga útil المخزن، ويؤكد حالة pendiente لضمان حتمية libro mayor. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| مولد نزاعات CLI يطابق المخطط القياسي | Utilice CLI para Base64/Norito y JSON para `CapacityDisputeV1`, y encontrará paquetes de pruebas disponibles. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| اختبار repetición يثبت حتمية النزاع/العقوبة | telemetría الخاصة بـ prueba-fallo التي تُعاد مرتين تنتج instantáneas متطابقة للـ libro mayor والائتمان والنزاع، ليبقى recorta حتميا بين pares. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| يوثق runbook مسار التصعيد والإلغاء | يلتقط دليل العمليات سير المجلس ومتطلبات الأدلة وإجراءات rollback. | `../dispute-revocation-runbook.md` |

### تسوية الخزانة| الفحص | التحقق | الدليل |
|-------|------------|----------|
| تراكم libro mayor يطابق توقع remojo لمدة 30 يوما | يمتد اختبار remojo عبر خمسة مزودين على 30 نافذة liquidación, مع مقارنة إدخالات libro mayor بالمرجع المتوقع للمدفوعات. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| تسوية صادرات libro mayor تُسجل ليلا | يقارن `capacity_reconcile.py` Registro de tarifas, registro de tarifas XOR y registro de tarifas Prometheus, y registro عبر Administrador de alertas. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Facturación, telemetría y telemetría | يعرض استيراد Grafana تراكم GiB-hour, عدادات strikes, والضمان المربوط لتمكين الرؤية لدى فريق المناوبة. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| التقرير المنشور يؤرشف منهجية remojo y repetición | يوضح التقرير نطاق remojo وأوامر التنفيذ وganchos للرصد من اجل المدققين. | `./sf2c-capacity-soak.md` |

## ملاحظات التنفيذ

أعد تشغيل حزمة التحقق قبل cierre de sesión:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

يجب على المشغلين إعادة توليد cargas útiles طلبات الانضمام/النزاع عبر `sorafs_manifest_stub capacity {declaration,dispute}` y بايتات JSON/Norito الناتجة بجانب تذكرة الحوكمة.

## artefactos الموافقة

| Artefacto | Camino | blake2b-256 |
|----------|------|-------------|
| حزمة موافقة انضمام المزودين | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| حزمة موافقة تسوية النزاعات | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| حزمة موافقة تسوية الخزانة | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

احتفظ بالنسخ الموقعة من هذه artefactos مع حزمة الإصدار واربطها في سجل تغييرات الحوكمة.

## الموافقات- Líder del equipo de almacenamiento: @storage-tl (24/03/2026)  
- Secretario del Consejo de Gobernación — @council-sec (24/03/2026)  
- Líder de operaciones de tesorería — @treasury-ops (24/03/2026)