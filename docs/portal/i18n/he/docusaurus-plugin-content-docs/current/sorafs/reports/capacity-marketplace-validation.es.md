---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Validacion del Mercado de capacidad SoraFS
תגיות: [SF-2c, קבלה, רשימת תיוג]
תקציר: רשימת קבלה לכניסה למטוס ספקים, פתרונות מחלוקת ופשרה על מנת לקבל גישה כללית ל-Mercado de capacidad SoraFS.
---

# Lista de Verificacion de Validacion del Mercado de capacidad SoraFS

**Ventana de revision:** 2026-03-18 -> 2026-03-24  
**אחראים על תוכנית:** צוות אחסון (`@storage-wg`), מועצת ממשל (`@council`), גילדת האוצר (`@treasury`)  
**Alcance:** צינורות לכניסה למטוסים של ספקים, תהליכי הכרעת מחלוקת ותהליכי פשרה של דרישות טסוריה ל-GA SF-2c.

רשימת הבדיקה הבאה לחידוש השוק לפני מפעילים חיצוניים. Cada fila enlaza evidencia deterministica (בדיקות, אביזרים או תיעוד) que los auditores pueden reproducerer.

## רשימת רשימת קבלה

### כניסה לספקים

| צ'קאו | Validacion | Evidencia |
|-------|----------------|--------|
| El registry acepta declaraciones canonicas de capacidad | בדיקת אינטגרציה בעלות בעלות `/v2/sorafs/capacity/declare` באמצעות API של אפליקציה, אימות אל ניהול החברות, קלטת מטא נתונים והעברת מטא נתונים ברישום. | `crates/iroha_torii/src/routing.rs:7654` |
| El smart contract rechaza payloads desalineados | El test unitario asegura que los IDs de provider y los campos de GiB comprometidos coinciden con la declaracion firmada antes de persistir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| El CLI emite artefactos canonicos de onboarding | הרתמה ל-CLI תרשום את הסלולר Norito/JSON/Base64 לקבוע נסיעות הלוך ושוב עבור מבצעי הפעלה או הכרזה במצב לא מקוון. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| La guia de operadores captura el flujo de admision y los guards de gobernanza | La documentacion enumera el esquema declaracion, los defaults de policy y los pasos de revision para el Council. | `../storage-capacity-marketplace.md` |

### פתרון מחלוקת

| צ'קאו | Validacion | Evidencia |
|-------|----------------|--------|
| רישומי מחלוקת מתמשכים עם קנוניקו מטען | בדיקת יחידת הרישום אינה מחלוקת, ביטול מטען מטען ותאגיד אל תעודת החשבון עבור חשבונית החשבונות. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| El generador de disputas del CLI coincide con el esquema canonico | El test del CLI cubre salidas Base64/Norito y resumenes JSON para `CapacityDisputeV1`, asegurando que los ראיות חבילות hashean de forma deterministica. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| El test de replay prueba el determinismo de disputa/penalizacion | La telemetry de proof-failure reproducida dos veces לייצר צילומי מצב זהות פנקס, קרדיט y disputas para que los slashes sean deterministas entre peers. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| El runbook documenta el flujo de escalamiento y revocacion | La guia de operaciones captura el flujo del Council, requisitos de Evidencia y procedimientos de rollback. | `../dispute-revocation-runbook.md` |### Conciliacion de tesoreria

| צ'קאו | Validacion | Evidencia |
|-------|----------------|--------|
| La acumulacion del ספר coincide con la proyeccion de soak de 30 dias | El soak test abarca cinco providers en 30 ventanas de settlement, comparando entradas del Ledger con la referencia de payout esperada. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| La conciliacion de exportes del financier se registra cada noche | `capacity_reconcile.py` השוואת מחירי פנקס אגרות עם יצוא XOR ejecutados, פליט מדדים Prometheus y gatea la aprobacion de tesoreria דרך Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| לוחות המחוונים של חיוב החשבון עונשין וטלמטריה של אקומולציה | אל ייבוא ​​של Grafana גרפיקה צבירת GiB-שעה, תומכי שביתות ובטחונות מלוכדים לראייה בכוננות. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| El reporte publicado archiva la metodologia del soak y comandos de replay | El reporte detalla el alcance del soak, comandos de ejecucion y hooks de observabilidad para auditores. | `./sf2c-capacity-soak.md` |

## הערות פליטות

Reejecuta la suite de validacion לפני היציאה:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

מפעילי החשבון מחדש את המטענים המשמשים ל-Onboarding/Disputa עם `sorafs_manifest_stub capacity {declaration,dispute}` עם ארכיון ב-JSON/Norito.

## Artefactos de aprobacion

| Artefacto | רוטה | blake2b-256 |
|--------|------|--------|
| Paquete de aprobacion de onboarding de providers | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Paquete de aprobacion de resolucion de disputas | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Paquete de aprobacion de conciliacion de tesoreria | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Guarda las copias firmadas de estos artefactos con el bundle de release y enlazalas en el registro de cambios de gobernanza.

## אפרובאציונס

- Lider del equipo de Storage — @storage-tl (2026-03-24)  
- Secretaria del Governance Council — @council-sec (2026-03-24)  
- Lider de Operaciones de Tesoreria — @treasury-ops (2026-03-24)