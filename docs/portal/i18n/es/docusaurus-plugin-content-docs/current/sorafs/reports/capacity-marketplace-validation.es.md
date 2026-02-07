---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Validación del mercado de capacidad SoraFS
Etiquetas: [SF-2c, aceptación, lista de verificación]
resumen: Checklist de aceptación que cubre onboarding de proveedores, flujos de disputa y conciliación de tesoreria que habilitan la disponibilidad general del mercado de capacidad SoraFS.
---

# Lista de verificación de validación del mercado de capacidad SoraFS

**Ventana de revisión:** 2026-03-18 -> 2026-03-24  
**Responsables del programa:** Equipo de Almacenamiento (`@storage-wg`), Consejo de Gobernanza (`@council`), Gremio de Tesorería (`@treasury`)  
**Alcance:** Pipelines de onboarding de proveedores, flujos de adjudicación de disputas y procesos de conciliación de tesorería requeridos para la GA SF-2c.

La lista de verificación siguiente debe revisarse antes de habilitar el mercado para operadores externos. Cada fila enlaza evidencia determinística (pruebas, accesorios o documentación) que los auditores pueden reproducir.

## Lista de verificación de aceptación

### Incorporación de proveedores| Chequeo | Validación | Pruebas |
|-------|------------|----------|
| El registro acepta declaraciones canónicas de capacidad | El test de integracion ejercita `/v1/sorafs/capacity/declare` via la app API, verificando el manejo de firmas, la captura de metadatos y el handoff al registro del nodo. | `crates/iroha_torii/src/routing.rs:7654` |
| El contrato inteligente rechaza cargas útiles desalineadas | El test unitario asegura que los ID del proveedor y los campos de GiB comprometidos coinciden con la declaración firmada antes de persistir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| El CLI emite artefactos canónicos de onboarding | El arnés de CLI escribe salidas Norito/JSON/Base64 determinísticas y valida viajes de ida y vuelta para que los operadores puedan preparar declaraciones fuera de línea. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| La guía de operadores captura el flujo de admisión y los guardrails de gobernanza | La documentación enumera el esquema de declaración, los valores predeterminados de política y los pasos de revisión para el consejo. | `../storage-capacity-marketplace.md` |

### Resolución de disputas| Chequeo | Validación | Pruebas |
|-------|------------|----------|
| Los registros de disputa persisten con el resumen canónico de carga útil | El test unitario registra una disputa, decodifica la carga útil almacenada y afirma el estado pendiente para garantizar el determinismo del libro mayor. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| El generador de disputas del CLI coincide con el esquema canónico | La prueba del CLI cubre salidas Base64/Norito y resúmenes JSON para `CapacityDisputeV1`, asegurando que los paquetes de evidencia hashean de forma determinística. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| El test de repetición prueba el determinismo de disputa/penalización | La telemetría de prueba-fallo reproducida dos veces produce instantáneas idénticas de libro mayor, créditos y disputas para que los cortes sean deterministas entre pares. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| El runbook documenta el flujo de escalamiento y revocacion | La guía de operaciones captura el flujo del consejo, requisitos de evidencia y procedimientos de rollback. | `../dispute-revocation-runbook.md` |

### Conciliación de tesorería| Chequeo | Validación | Pruebas |
|-------|------------|----------|
| La acumulación del libro mayor coincide con la proyección de remojo de 30 días | La prueba de remojo abarca cinco proveedores en 30 ventanas de liquidación, comparando entradas del libro mayor con la referencia de pago esperado. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| La conciliacion de exportaciones del libro mayor se registra cada noche | `capacity_reconcile.py` compara expectativas del fee ledger con exportes XOR ejecutados, emite métricas Prometheus y gatea la aprobacion de tesoreria via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Los paneles de facturación exponen penalizaciones y telemetría de acumulación | El import de Grafana acumulación gráfica GiB-hora, contadores de strikes y colateral bonded para visibilidad on-call. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| El reporte publicado archiva la metodología del remojo y comandos de repetición | El reporte detalla el alcance del remojo, comandos de ejecución y ganchos de observabilidad para auditores. | `./sf2c-capacity-soak.md` |

## Notas de ejecucion

Reejecuta la suite de validación antes del sign-off:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Los operadores deben regenerar las cargas útiles de solicitud de onboarding/disputa con `sorafs_manifest_stub capacity {declaration,dispute}` y archivar los bytes JSON/Norito resultantes junto al ticket de gobernanza.

## Artefactos de aprobación| Artefacto | Ruta | blake2b-256 |
|----------|------|-------------|
| Paquete de aprobación de incorporación de proveedores | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Paquete de aprobación de resolución de disputas | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Paquete de aprobación de conciliación de tesorería | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Guarde las copias firmadas de estos artefactos con el paquete de liberación y enlacelas en el registro de cambios de gobernanza.

##Aprobaciones

- Líder del equipo de Storage — @storage-tl (2026-03-24)  
- Secretaria del Consejo de Gobernación — @council-sec (2026-03-24)  
- Líder de Operaciones de Tesorería — @treasury-ops (2026-03-24)