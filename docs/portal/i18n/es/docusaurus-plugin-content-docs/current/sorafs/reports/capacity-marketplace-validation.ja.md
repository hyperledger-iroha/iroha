---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b980d966ef2ed6ed38f95534e90d19d6e6feb936ead0d3988aa5e3c6d5a02c8
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Lista de verificacion de validacion del mercado de capacidad SoraFS

**Ventana de revision:** 2026-03-18 -> 2026-03-24  
**Responsables del programa:** Storage Team (`@storage-wg`), Governance Council (`@council`), Treasury Guild (`@treasury`)  
**Alcance:** Pipelines de onboarding de providers, flujos de adjudicacion de disputas y procesos de conciliacion de tesoreria requeridos para la GA SF-2c.

La checklist siguiente debe revisarse antes de habilitar el marketplace para operadores externos. Cada fila enlaza evidencia deterministica (tests, fixtures o documentacion) que los auditores pueden reproducir.

## Checklist de aceptacion

### Onboarding de providers

| Chequeo | Validacion | Evidencia |
|-------|------------|----------|
| El registry acepta declaraciones canonicas de capacidad | El test de integracion ejercita `/v1/sorafs/capacity/declare` via la app API, verificando el manejo de firmas, la captura de metadata y el handoff al registry del nodo. | `crates/iroha_torii/src/routing.rs:7654` |
| El smart contract rechaza payloads desalineados | El test unitario asegura que los IDs de provider y los campos de GiB comprometidos coinciden con la declaracion firmada antes de persistir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| El CLI emite artefactos canonicos de onboarding | El harness de CLI escribe salidas Norito/JSON/Base64 deterministicas y valida round-trips para que los operadores puedan preparar declaraciones offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| La guia de operadores captura el flujo de admision y los guardrails de gobernanza | La documentacion enumera el esquema de declaracion, los defaults de policy y los pasos de revision para el council. | `../storage-capacity-marketplace.md` |

### Resolucion de disputas

| Chequeo | Validacion | Evidencia |
|-------|------------|----------|
| Los registros de disputa persisten con digest canonico de payload | El test unitario registra una disputa, decodifica el payload almacenado y afirma el estado pendiente para garantizar determinismo del ledger. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| El generador de disputas del CLI coincide con el esquema canonico | El test del CLI cubre salidas Base64/Norito y resumenes JSON para `CapacityDisputeV1`, asegurando que los evidence bundles hashean de forma deterministica. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| El test de replay prueba el determinismo de disputa/penalizacion | La telemetry de proof-failure reproducida dos veces produce snapshots identicos de ledger, creditos y disputas para que los slashes sean deterministas entre peers. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| El runbook documenta el flujo de escalamiento y revocacion | La guia de operaciones captura el flujo del council, requisitos de evidencia y procedimientos de rollback. | `../dispute-revocation-runbook.md` |

### Conciliacion de tesoreria

| Chequeo | Validacion | Evidencia |
|-------|------------|----------|
| La acumulacion del ledger coincide con la proyeccion de soak de 30 dias | El soak test abarca cinco providers en 30 ventanas de settlement, comparando entradas del ledger con la referencia de payout esperada. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| La conciliacion de exportes del ledger se registra cada noche | `capacity_reconcile.py` compara expectativas del fee ledger con exportes XOR ejecutados, emite metricas Prometheus y gatea la aprobacion de tesoreria via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Los dashboards de billing exponen penalizaciones y telemetry de acumulacion | El import de Grafana grafica acumulacion GiB-hour, contadores de strikes y collateral bonded para visibilidad on-call. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| El reporte publicado archiva la metodologia del soak y comandos de replay | El reporte detalla el alcance del soak, comandos de ejecucion y hooks de observabilidad para auditores. | `./sf2c-capacity-soak.md` |

## Notas de ejecucion

Reejecuta la suite de validacion antes del sign-off:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Los operadores deben regenerar los payloads de solicitud de onboarding/disputa con `sorafs_manifest_stub capacity {declaration,dispute}` y archivar los bytes JSON/Norito resultantes junto al ticket de gobernanza.

## Artefactos de aprobacion

| Artefacto | Ruta | blake2b-256 |
|----------|------|-------------|
| Paquete de aprobacion de onboarding de providers | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Paquete de aprobacion de resolucion de disputas | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Paquete de aprobacion de conciliacion de tesoreria | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Guarda las copias firmadas de estos artefactos con el bundle de release y enlazalas en el registro de cambios de gobernanza.

## Aprobaciones

- Lider del equipo de Storage — @storage-tl (2026-03-24)  
- Secretaria del Governance Council — @council-sec (2026-03-24)  
- Lider de Operaciones de Tesoreria — @treasury-ops (2026-03-24)
