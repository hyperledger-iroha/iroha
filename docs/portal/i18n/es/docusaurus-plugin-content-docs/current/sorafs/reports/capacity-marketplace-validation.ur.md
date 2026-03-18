---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Validación del mercado de capacidad SoraFS
resumen: lista de verificación de aceptación, incorporación de proveedores, flujos de trabajo de disputas, conciliación de tesorería, mercado de capacidad de GA, puerta de enlace, SoraFS.
Etiquetas: [SF-2c, aceptación, lista de verificación]
---

# SoraFS Lista de verificación de validación del mercado de capacidad

**Ventana de revisión:** 2026-03-18 -> 2026-03-24  
**Propietarios del programa:** Equipo de almacenamiento (`@storage-wg`), Consejo de gobierno (`@council`), Gremio de tesorería (`@treasury`)  
**Alcance:** Canalizaciones de incorporación de proveedores, flujos de adjudicación de disputas, procesos de conciliación de tesorería, SF-2c GA کے لئے درکار ہیں۔

نیچے دی گئی lista de verificación کو بیرونی operadores کے لئے mercado habilitar کرنے سے پہلے revisión کرنا لازم ہے۔ ہر fila de evidencia determinista (pruebas, accesorios, یا documentación) سے لنک کرتی ہے جسے los auditores repiten کر سکتے ہیں۔

## Lista de verificación de aceptación

### Incorporación de proveedores| Consultar | Validación | Evidencia |
|-------|------------|----------|
| Declaraciones de capacidad canónica del Registro قبول کرتا ہے | Aplicación de prueba de integración API کے ذریعے `/v1/sorafs/capacity/declare` چلایا جاتا ہے، manejo de firmas, captura de metadatos, اور registro de nodos تک traspaso کو verificar کرتا ہے۔ | `crates/iroha_torii/src/routing.rs:7654` |
| Cargas útiles no coincidentes de contratos inteligentes کو rechazar کرتا ہے | Prueba unitaria یقینی بناتا ہے کہ ID de proveedor اور campos GiB comprometidos declaración firmada کے مطابق ہوں قبل از persistencia۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| Los artefactos de incorporación canónicos de CLI emiten کرتا ہے | CLI aprovecha las salidas deterministas Norito/JSON/Base64 لکھتا ہے اور ida y vuelta validar کرتا ہے تاکہ operadores declaraciones fuera de línea تیار کر سکیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Flujo de trabajo de admisión de la guía del operador اور barandillas de gobierno کو portada کرتا ہے | Esquema de declaración de documentación, valores predeterminados de políticas, pasos de revisión del consejo y enumerar کرتی ہے۔ | `../storage-capacity-marketplace.md` |

### Resolución de disputas| Consultar | Validación | Evidencia |
|-------|------------|----------|
| Registros de disputas resumen de carga útil canónica کے ساتھ persisten ہوتے ہیں | Registro de disputas de prueba unitaria کرتا ہے، decodificación de carga útil almacenada کرتا ہے، اور afirmación de estado pendiente کرتا ہے تاکہ determinismo del libro mayor یقینی ہو۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Esquema canónico del generador de disputas CLI سے coincidencia کرتا ہے | Prueba CLI `CapacityDisputeV1` کے لئے Salidas Base64/Norito اور JSON resúmenes cubren کرتا ہے، جس سے paquetes de evidencia hash determinista ہوتے ہیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Repetición de prueba disputa/determinismo de penalización کو ثابت کرتا ہے | Telemetría de prueba de fallo کو دو بار repetición کرنے سے libro mayor, crédito اور instantáneas de disputas یکساں بنتے ہیں تاکہ recorta a sus pares کے درمیان determinista رہیں۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Escalada de runbook y flujo de revocación کو documento کرتا ہے | Flujo de trabajo del consejo de guía de operaciones, requisitos de evidencia y procedimientos de reversión y captura de datos | `../dispute-revocation-runbook.md` |

### Conciliación de Tesorería| Consultar | Validación | Evidencia |
|-------|------------|----------|
| Proyección de absorción de 30 días de acumulación del libro mayor سے coincidencia کرتا ہے | Prueba de remojo پانچ proveedores کو 30 ventanas de liquidación میں چلاتا ہے، asientos contables کو referencia de pago esperado کے ساتھ diff کرتا ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Conciliación de exportaciones del libro mayor ہر رات registro ہوتا ہے | `capacity_reconcile.py` expectativas del libro mayor de tarifas کو exportaciones de transferencias XOR ejecutadas کے ساتھ comparar کرتا ہے، Las métricas Prometheus emiten کرتا ہے، اور Alertmanager کے ذریعے puerta de aprobación de tesorería کرتا ہے۔ | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Penalizaciones de paneles de facturación اور superficie de telemetría de acumulación کرتے ہیں | Grafana importar acumulación de horas GiB, contadores de huelgas, gráfico de garantía garantizada کرتا ہے تاکہ visibilidad de guardia ملے۔ | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Metodología de remojo de informes publicados اور archivo de comandos de reproducción کرتا ہے | Informe de alcance de remojo, comandos de ejecución, ganchos de observabilidad, auditores, detalles, detalles | `./sf2c-capacity-soak.md` |

## Notas de ejecución

Aprobación de la suite de validación دوبارہ چلائیں:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Operadores `sorafs_manifest_stub capacity {declaration,dispute}` cargas útiles de solicitud de incorporación/disputa generan bytes JSON/Norito ticket de gobernanza ساتھ archivo کرنا چاہیے۔

## Artefactos de aprobación| Artefacto | Camino | blake2b-256 |
|----------|------|-------------|
| Paquete de aprobación de incorporación de proveedores | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Paquete de aprobación de resolución de disputas | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Paquete de aprobación de conciliación del Tesoro | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

ان artefactos کی copias firmadas کو paquete de lanzamiento کے ساتھ محفوظ کریں اور registro de cambios de gobernanza میں لنک کریں۔

## Aprobaciones

- Líder del equipo de almacenamiento: @storage-tl (24/03/2026)  
- Secretario del Consejo de Gobernación — @council-sec (24/03/2026)  
- Líder de operaciones de tesorería — @treasury-ops (24/03/2026)