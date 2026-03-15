---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Validacao do mercado de capacidade SoraFS
Etiquetas: [SF-2c, aceptación, lista de verificación]
resumen: Lista de verificación de aceitacao cobrindo onboarding de proveedores, flujos de disputa e reconciliacao do tesouro que liberam a disponibilidade geral do mercado de capacidade SoraFS.
---

# Lista de verificación de validación del mercado de capacidad SoraFS

**Janela de revisión:** 2026-03-18 -> 2026-03-24  
**Responsaveis do programa:** Equipo de Almacenamiento (`@storage-wg`), Consejo de Gobierno (`@council`), Gremio de Tesorería (`@treasury`)  
**Escopo:** Canalizaciones de incorporación de proveedores, flujos de adjudicación de disputas y procesos de reconciliación del tesoro requeridos para un GA SF-2c.

La lista de verificación a continuación debe ser revisada antes de habilitar el mercado para operadores externos. Cada línea conecta evidencias determinísticas (pruebas, accesorios o documentación) que los auditores pueden reproducir.

## Lista de verificación de aceitacao

### Incorporación de proveedores| Checagem | Validacao | Pruebas |
|-------|------------|----------|
| O registro aceita declaracoes canónicas de capacidade | La prueba de integración ejercita `/v2/sorafs/capacity/declare` a través de la aplicación API, verificando el tratamiento de assinaturas, captura de metadatos y transferencia para el registro del nodo. | `crates/iroha_torii/src/routing.rs:7654` |
| O contrato inteligente rejeita cargas útiles divergentes | O teste unitario garante que ID de proveedor y campos GiB confirmados corresponden a declaracao assinada antes de persistir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| O CLI emite artefatos canónicos de onboarding | El arnés de CLI escribe las indicaciones Norito/JSON/Base64 determinísticas y valida viajes de ida y vuelta para que los operadores puedan preparar declaraciones fuera de línea. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| La guía del operador de cobre o el flujo de trabajo de admisión y las barandillas de gobierno | La documentación enumera los esquemas de declaración, los valores predeterminados de política y los pasos de revisión para el consejo. | `../storage-capacity-marketplace.md` |

### Resolución de disputas| Checagem | Validacao | Pruebas |
|-------|------------|----------|
| Registros de disputa persistentes con resumen canónico de carga útil | O teste unitario registra uma disputa, decodifica o payload armazenado y afirma status pendiente para garantizar el determinismo del libro mayor. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| El generador de disputas de CLI corresponde al esquema canónico | La prueba de cobre CLI dice Base64/Norito y resumos JSON para `CapacityDisputeV1`, lo que garantiza que los paquetes de evidencia tienen hash determinístico. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| O teste de replay prueba determinismo de disputa/penalidade | Una telemetría de prueba-fallo reproducida dos veces genera instantáneas idénticas de libro mayor, créditos y disputas para que cortes sejam determinísticos entre pares. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| El runbook documenta el flujo de escalamiento y revogacao | La guía de operaciones de captura o flujo de trabajo del consejo, requisitos de evidencia y procedimientos de reversión. | `../dispute-revocation-runbook.md` |

### Reconciliacao do tesouro| Checagem | Validacao | Pruebas |
|-------|------------|----------|
| La acumulación del libro mayor corresponde a un proyecto de remojo de 30 días | La prueba de remojo ofrece cinco proveedores en 30 días de liquidación, comparando las entradas del libro mayor con una referencia de pago esperado. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| A reconciliacao de exportes do ledger e registrado a cada noite | `capacity_reconcile.py` compara las expectativas del fee ledger com exportes XOR ejecutados, emite métricas Prometheus y faz gate da aprovacao do tesouro via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Paneles de facturación expoem penalidades y telemetría de acumulación | La importación de Grafana trama o acumulación de GiB-hora, contadores de huelgas y garantías garantizadas para visibilidad de guardia. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| El relatorio publicado archiva sobre metodología de remojo y comandos de repetición | El relato detallado del alcance del remojo, los comandos de ejecución y los ganchos de observabilidad para auditores. | `./sf2c-capacity-soak.md` |

## Notas de ejecución

Vuelva a ejecutar una suite de validación antes de cerrar sesión:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Los operadores deben regenerar cargas útiles de solicitud de onboarding/disputa con `sorafs_manifest_stub capacity {declaration,dispute}` y archivar los bytes JSON/Norito resultantes junto al ticket de gobierno.

## Artefatos de aprovacao| Artefacto | Camino | blake2b-256 |
|----------|------|-------------|
| Pacote de aprobación de incorporación de proveedores | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Pacote de aprobación de resolución de disputas | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Pacote de aprovacao de reconciliacao do tesouro | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Guarde as copias assinadas desses artefatos com o bundle de release e vincule-as no registro de mudancas degobernanza.

## Aprovacoes

- Líder del equipo de almacenamiento - @storage-tl (24/03/2026)  
- Secretario del Consejo de Gobernación - @council-sec (24-03-2026)  
- Líder de Operaciones de Tesorería - @treasury-ops (24/03/2026)