---
lang: es
direction: ltr
source: docs/source/nexus_compliance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5635794e962a9fb1b94c5ff550dc198a744a64b4f9f05df588cb70621e9237f9
source_last_modified: "2025-11-21T18:31:26.542844+00:00"
translation_last_reviewed: 2026-01-01
---

# Motor de cumplimiento de lanes Nexus y politica de lista blanca (NX-12)

Estado: Implementado — este documento captura el modelo de politica en vivo y la aplicacion critica
para consenso referenciada por el elemento de roadmap **NX-12 - Motor de cumplimiento de lane y politica de lista blanca**.
Explica el modelo de datos, los flujos de gobernanza, la telemetria y la estrategia de despliegue
implementados dentro de `crates/iroha_core/src/compliance` y aplicados tanto en la
admision de Torii como en la validacion de transacciones de `iroha_core`, para que cada lane y dataspace pueda quedar ligado a
politicas jurisdiccionales deterministas.

## Objetivos

- Permitir que la gobernanza adjunte reglas allow/deny, flags jurisdiccionales, limites de transferencia CBDC
  y requisitos de auditoria a cada manifiesto de lane.
- Evaluar cada transaccion contra esas reglas durante la admision en Torii y la ejecucion del bloque,
  asegurando la aplicacion determinista de la politica en todos los nodos.
- Producir una traza de auditoria criptograficamente verificable, con paquetes de evidencia Norito
  y telemetria consultable para reguladores y operadores.
- Mantener el modelo flexible: el mismo motor de politicas cubre lanes CBDC privadas,
  DS de settlement publicas y dataspaces hibridos de socios sin forks a medida.

## No-Objetivos

- Definir procedimientos AML/KYC o flujos legales de escalado. Eso vive en los playbooks de cumplimiento
  que consumen la telemetria producida aqui.
- Introducir toggles por instruccion en IVM; el motor solo controla que cuentas/activos/domains
  pueden enviar transacciones o interactuar con una lane.
- Hacer obsoleto Space Directory. Los manifiestos siguen siendo la fuente autoritativa
  de metadatos de DS; la politica de cumplimiento solo referencia entradas de Space Directory
  y las complementa.

## Modelo de politica

### Entidades e identificadores

El motor de politicas opera sobre:

- `LaneId` / `DataSpaceId` - identifica el alcance donde aplican las reglas.
- `UniversalAccountId (UAID)` - permite agrupar identidades cross-lane.
- `JurisdictionFlag` - bitmask que enumera clasificaciones regulatorias (p. ej.
  `EU_EEA`, `JP_FIEL`, `US_FED`, `SANCTIONS_SCREENED`).
- `ParticipantSelector` - describe a quien afecta:
  - `AccountId`, `DomainId` o `UAID`.
  - Selectores basados en prefijo (`DomainPrefix`, `UaidPrefix`) para coincidir con registros.
  - `CapabilityTag` para manifiestos de Space Directory (p. ej. solo DS con FX-cleared).
  - gating `privacy_commitments_any_of` para exigir que las lanes anuncien compromisos de privacidad Nexus
    especificos antes de que las reglas coincidan (refleja la superficie de manifiesto NX-10
    y se aplica en snapshots de `LanePrivacyRegistry`).

### LaneCompliancePolicy

Las politicas son structs Norito publicados via gobernanza:

```text
LaneCompliancePolicy {
    id: LaneCompliancePolicyId,
    version: u32,
    lane_id: LaneId,
    jurisdiction: JurisdictionSet,
    allow: Vec<AllowRule>,
    deny: Vec<DenyRule>,
    transfer_limits: Vec<TransferLimit>,
    audit_controls: AuditControls,
    metadata: MetadataMap,
}
```

- `AllowRule` combina un `ParticipantSelector`, override jurisdiccional opcional,
  capability tags y codigos de motivo.
- `DenyRule` refleja la estructura de allow pero se evalua primero (deny gana).
- `TransferLimit` captura limites especificos por activo/bucket:
  - `max_notional_xor` y `max_daily_notional_xor`.
  - `asset_limits[{asset_id, per_tx, per_day}]`.
  - `relationship_limits` (p. ej. CBDC retail vs wholesale).
- `AuditControls` configura:
  - Si Torii debe persistir cada denegacion en el log de auditoria.
  - Si las decisiones exitosas deben muestrearse en digests Norito.
  - Ventana de retencion requerida para `LaneComplianceDecisionRecord`.

### Almacenamiento y distribucion

- Los hashes de politica mas recientes viven en el manifiesto de Space Directory junto con las llaves
  de validadores. `LaneCompliancePolicyReference` (policy id + version + hash) se convierte en un
  campo de manifiesto para que validadores y SDKs puedan obtener el blob de politica canonico.
- `iroha_config` expone `compliance.policy_cache_dir` para persistir el payload Norito y su firma
  separada. Los nodos verifican firmas antes de aplicar actualizaciones para protegerse contra
  manipulacion.
- Las politicas tambien se incrustan en los manifiestos de admision Norito usados por Torii
  para que CI/SDKs puedan reproducir la evaluacion de politicas sin hablar con validadores.

## Gobernanza y ciclo de vida

1. **Propuesta** - la gobernanza envia `ProposeLaneCompliancePolicy` con el payload Norito,
   la justificacion jurisdiccional y la epoca de activacion.
2. **Revision** - los revisores de cumplimiento firman `LaneCompliancePolicyReviewEvidence`
   (auditable, almacenado en `governance::ReviewEvidenceStore`).
3. **Activacion** - despues de la ventana de espera, los validadores incorporan la politica
   llamando a `ActivateLaneCompliancePolicy`. El manifiesto de Space Directory se actualiza de forma
   atomica con la nueva referencia de politica.
4. **Enmienda/Revocacion** - `AmendLaneCompliancePolicy` lleva metadata de diff mientras mantiene la
   version previa para replay forense; `RevokeLaneCompliancePolicy` fija el policy id en `denied`
   para que Torii rechace cualquier trafico dirigido a esa lane hasta que se active un reemplazo.

Torii expone:

- `GET /v2/lane-compliance/policies/{lane_id}` - obtiene la referencia de politica mas reciente.
- `POST /v2/lane-compliance/policies` - endpoint solo gobernanza que refleja los helpers de propuesta ISI.
- `GET /v2/lane-compliance/decisions` - log de auditoria paginado con filtros para
  `lane_id`, `decision`, `jurisdiction` y `reason_code`.

Los comandos CLI/SDK envuelven esas superficies HTTP para que los operadores puedan automatizar
revisiones y obtener artefactos (blob de politica firmado + atestaciones de revisores).

## Pipeline de enforcement

1. **Admision (Torii)**
   - `Torii` descarga la politica activa cuando cambia un manifiesto de lane o cuando expira
     la firma en cache.
   - Cada transaccion que entra en la cola `/v2/pipeline` se etiqueta con
     `LaneComplianceContext` (ids de participante, UAID, metadatos del manifiesto de dataspace,
     policy id y el snapshot mas reciente de `LanePrivacyRegistry` descrito en
     `crates/iroha_core/src/interlane/mod.rs`).
   - Las autoridades con UAID deben tener un manifiesto activo de Space Directory para el dataspace
     enrutado; Torii rechaza transacciones cuando el UAID no esta vinculado a ese dataspace antes
     de evaluar cualquier regla de politica.
   - El `compliance::Engine` evalua reglas `deny`, luego reglas `allow`, y finalmente aplica limites
     de transferencia. Las transacciones fallidas devuelven un error tipado
     (`ERR_LANE_COMPLIANCE_DENIED`) con motivo + policy id para trazas de auditoria.
   - La admision es un prefiltro rapido; la validacion de consenso vuelve a comprobar las mismas
     reglas usando snapshots del estado para mantener la aplicacion determinista.
2. **Ejecucion (iroha_core)**
   - Durante la construccion del bloque, `iroha_core::tx::validate_transaction_internal`
     reproduce las mismas comprobaciones de gobernanza/UAID/privacidad/cumplimiento de lane usando los
     snapshots de `StateTransaction` (`lane_manifests`, `lane_privacy_registry`,
     `lane_compliance`). Esto mantiene la aplicacion critica para consenso incluso si los caches de
     Torii quedan obsoletos.
   - Las transacciones que mutan manifiestos de lane o politicas de cumplimiento siguen el mismo
     camino de validacion; no hay bypass solo de admision.
3. **Hooks asincronos**
   - RBC gossip y los fetchers de DA adjuntan el policy id a la telemetria para que decisiones tardias
     puedan rastrearse a la version correcta de reglas.
   - `iroha_cli` y los helpers SDK exponen `LaneComplianceDecision::explain()` para que la automatizacion
     pueda renderizar diagnosticos legibles.

El motor es determinista y puro; nunca se comunica con sistemas externos despues de descargar
el manifiesto/politica. Eso mantiene los fixtures de CI y la reproduccion multi-nodo directos.

## Auditoria y telemetria

- **Metricas**
  - `nexus_lane_policy_decisions_total{lane_id,decision,reason}`.
  - `nexus_lane_policy_rate_limited_total{lane_id,limit_kind}`.
  - `nexus_lane_policy_cache_age_seconds{lane_id}` (debe mantenerse < retardo de activacion).
- **Logs**
  - Registros estructurados capturan `policy_id`, `version`, `participant`, `UAID`,
    flags jurisdiccionales y el hash Norito de la transaccion infractora.
  - `LaneComplianceDecisionRecord` se codifica en Norito y se persiste bajo
    `world.compliance_logs::<lane_id>::<ts>::<nonce>` cuando `AuditControls`
    solicita almacenamiento duradero.
- **Paquetes de evidencia**
  - `cargo xtask nexus-lane-audit` gana un modo `--lane-compliance <path>` que fusiona la politica,
    firmas de revisores, snapshot de metricas y el log de auditoria mas reciente en las salidas
    JSON + Parquet. El flag espera un payload JSON con la forma:

    ```json
    {
      "lanes": [
        {
          "lane_id": 12,
          "policy": { "...": "LaneCompliancePolicy JSON blob" },
          "reviewer_signatures": [
            {
              "reviewer": "auditor@example.com",
              "signature_hex": "deadbeef",
              "signed_at": "2026-02-12T09:00:00Z",
              "notes": "Q1 regulator packet"
            }
          ],
          "metrics_snapshot": {
            "nexus_lane_policy_decisions_total": {
              "allow": 42,
              "deny": 1
            }
          },
          "audit_log": [
            {
              "decision": "allow",
              "policy_id": "lane-12-policy",
              "recorded_at": "2026-02-12T09:00:00Z"
            }
          ]
        }
      ]
    }
    ```

    La CLI valida que cada blob `policy` coincida con el `lane_id` listado en el registro antes de
    insertarlo, evitando evidencia obsoleta o desalineada en paquetes regulatorios y dashboards del roadmap.
  - `--markdown-out` (por defecto `artifacts/nexus_lane_audit.md`) ahora renderiza un resumen legible
    que resalta lanes rezagadas, backlog no cero, manifiestos pendientes y evidencia de cumplimiento
    faltante para que los paquetes annex incluyan tanto artefactos machine-readable como una superficie
    rapida de revision.

## Plan de despliegue

1. **P0 - Solo observabilidad**
   - Enviar los tipos de politica, almacenamiento, endpoints de Torii y metricas.
   - Torii evalua politicas en modo `audit` (sin enforcement) para recolectar datos.
2. **P1 - Enforcement de deny/allow**
   - Habilitar fallos duros en Torii + ejecucion cuando se disparan reglas deny.
   - Requerir politicas para todas las lanes CBDC; los DS publicos pueden seguir en modo audit.
3. **P2 - Limites y overrides jurisdiccionales**
   - Activar enforcement de limites de transferencia y flags jurisdiccionales.
   - Alimentar la telemetria en `dashboards/grafana/nexus_lanes.json`.
4. **P3 - Automatizacion completa de cumplimiento**
   - Integrar exportes de auditoria con consumidores de `SpaceDirectoryEvent`.
   - Vincular actualizaciones de politica a runbooks de gobernanza y automatizacion de releases.

## Aceptacion y pruebas

- Las pruebas de integracion en `integration_tests/tests/nexus/compliance.rs` cubren:
  - combinaciones allow/deny, overrides jurisdiccionales y limites de transferencia;
  - carreras de activacion de manifiesto/politica; y
  - paridad de decision Torii vs `iroha_core` en ejecuciones multi-nodo.
- Las pruebas unitarias en `crates/iroha_core/src/compliance` validan el motor de evaluacion puro,
  temporizadores de invalidacion de cache y parsing de metadatos.
- Las actualizaciones de Docs/SDK (Torii + CLI) deben demostrar la obtencion de politicas,
  envio de propuestas de gobernanza, interpretacion de codigos de error y recoleccion de evidencia
  de auditoria.

Cerrar NX-12 requiere los artefactos anteriores mas actualizaciones de estado en
`status.md`/`roadmap.md` una vez que el enforcement este activo en clusters de staging.
