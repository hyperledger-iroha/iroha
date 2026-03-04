---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Validación de la marcha de capacidad SoraFS
Etiquetas: [SF-2c, aceptación, lista de verificación]
Resumen: Lista de verificación de aceptación de la incorporación de proveedores, flujo de litigios y reconciliación del tresor condicionante de la disponibilidad general de la marcha de capacidad SoraFS.
---

# Lista de verificación de validación de la marcha de capacidad SoraFS

**Fenetre de revista:** 2026-03-18 -> 2026-03-24  
**Responsables del programa:** Equipo de almacenamiento (`@storage-wg`), Consejo de Gobierno (`@council`), Gremio de Tesorería (`@treasury`)  
**Porte:** Canalizaciones de incorporación de proveedores, flujo de adjudicación de litigios y proceso de reconciliación de tres requisitos para la GA SF-2c.

La lista de verificación ci-dessous debe ser revisada antes de activar la marcha para los operadores externos. Cada línea de envío vers una evidencia determinante (pruebas, accesorios o documentación) que los auditores pueden renovar.

## Lista de verificación de aceptación

### Incorporación de proveedores| Consultar | Validación | Evidencia |
|-------|------------|----------|
| El registro acepta las declaraciones canónicas de capacidad | La prueba de integración `/v1/sorafs/capacity/declare` a través de la API de la aplicación verifica la gestión de firmas, la captura de metadatos y la transferencia al registro del nodo. | `crates/iroha_torii/src/routing.rs:7654` |
| El contrato inteligente rechaza las cargas útiles incoherentes | La prueba unitaria garantiza que las identificaciones del proveedor y los campeones de GiB se comprometen con el correspondiente a la declaración del firmante antes de la persistencia. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| El CLI muestra los artefactos de incorporación canónica | El arnés CLI escrito de salidas Norito/JSON/Base64 determina y valida los viajes de ida y vuelta para que los operadores puedan preparar las declaraciones fuera de línea. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| El guía operador cubre el flujo de trabajo de admisión y los guardias de gobierno | La documentación enumera el esquema de declaración, los valores predeterminados de política y las etapas de revista para el consejo. | `../storage-capacity-marketplace.md` |

### Resolución de litigios| Consultar | Validación | Evidencia |
|-------|------------|----------|
| Los registros de litigios persistentes con un resumen canónico de la carga útil | La prueba unitaria registra un litigio, decodifica el stock de carga útil y afirma el estatuto pendiente para garantizar el determinismo del libro mayor. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| El generador de litigios del CLI corresponde al esquema canónico | La prueba CLI cubre las salidas Base64/Norito y los currículums JSON para `CapacityDisputeV1`, asegurándose de que los paquetes de pruebas tengan una forma determinante. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Le test de replay prueba el determinismo litige/penalité | La telemetría de prueba-fallo se activa dos veces al producir instantáneas idénticas de libro mayor, crédito y litigio porque las barras diagonales son determinantes entre pares. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| El runbook documenta el flujo de escalada y revocación | La guía de operaciones captura el flujo de trabajo del consejo, las exigencias de prevención y los procedimientos de reversión. | `../dispute-revocation-runbook.md` |

### Reconciliación del tresor| Consultar | Validación | Evidencia |
|-------|------------|----------|
| La acumulación del libro mayor corresponde a la proyección de remojo de 30 días | La prueba empapará a cinco proveedores en 30 ventanas de liquidación, en comparación con las entradas del libro mayor a la referencia de los asistentes de pago. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| La reconciliation des exports de ledger est registrada cada noche | `capacity_reconcile.py` compara los registros del libro mayor de tarifas aux exports XOR ejecuta, emet des métricas Prometheus y puerta la aprobación del tresor a través de Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Los paneles de facturación exponen las penalidades y la telemetría de acumulación | La importación Grafana rastrea la acumulación de GiB-hora, los computadores de huelga y la garantía se activan para la visibilidad de guardia. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Le rapport publice archive la metodología de remojo y los comandos de repetición | La relación detalla la puerta de remojo, los comandos de ejecución y los ganchos de observabilidad para los auditores. | `./sf2c-capacity-soak.md` |

## Notas de ejecución

Relance el conjunto de validación antes del cierre de sesión:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Los operadores deben regenerar las cargas útiles de demanda de incorporación/litigación con `sorafs_manifest_stub capacity {declaration,dispute}` y archivar los bytes JSON/Norito resultantes de las costas del ticket de gobierno.

## Artefactos de cierre de sesión| Artefacto | Camino | blake2b-256 |
|----------|------|-------------|
| Paquete de aprobación de incorporación de proveedores | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Paquete de aprobación de resolución de litigios | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Paquete de aprobación de reconciliación del tresor | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Conserve las copias firmadas de estos artefactos con el paquete de liberación y reliez-les dans le registre de changement de gouvernance.

## Aprobaciones

- Líder del equipo de almacenamiento: @storage-tl (24/03/2026)  
- Secretario del Consejo de Gobernación — @council-sec (24/03/2026)  
- Líder de operaciones de tesorería — @treasury-ops (24/03/2026)