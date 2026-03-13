---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_compliance_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3ea04f6ac217749839351798378d0eacea584d3236fb760edba3bdc685da995
source_last_modified: "2025-11-20T18:13:00.257988+00:00"
translation_last_reviewed: "2026-01-30"
---

# Cumplimiento, moderacion y transparencia del gateway

## Objetivos y alcance
- Equipar a los gateways SoraFS con una capa de compliance determinista que ingesta denylists externas, aplica toggles de moderacion controlados por operadores y registra cada decision para auditoria.
- Proveer reportes transparentes (semanales/mensuales) y proof tokens criptograficos para que los clientes verifiquen resultados de moderacion sin exponer detalles sensibles.
- Integrar con governance, workflows de apelaciones (SFM-4b), AI pre-screen (SFM-4a) y dashboards de transparencia (SFM-4c) manteniendo privacidad y cumplimiento legal.

Esta especificacion completa **SFM-4 — modulo de compliance y transparencia del gateway** a nivel arquitectonico; sub-workstreams (4a/4b/4c) complementan este plan con detalle especifico del dominio.

## Arquitectura (overview)
| Componente | Responsabilidad | Notas |
|-----------|------------------|-------|
| Compliance controller (`gateway_complianced`) | Fetch de denylists, normalizacion de entradas, aplica la matriz de politicas, distribuye updates a nodos gateway. | Corre centralmente por ambiente; publica updates firmados via Torii. |
| Modulo de enforcement en gateway (`gateway::compliance`) | Evalua requests/responses contra reglas activas, emite decisiones deny/allow, genera proof tokens + telemetria. | Embebido en cada binario gateway (Rust). |
| Servicio de toggles de moderacion (`moderation_toggle_api`) | UI/API de operador para habilitar/deshabilitar categorias de politica por gateway con audit trail. | Respaldado por Postgres audit log + aprobaciones multi-sig. |
| Transparency reporter (`transparency_reporter`) | Agrega eventos de moderacion, emite reportes semanales/mensuales, publica a Governance DAG + dashboards. | Escribe payloads Norito `TransparencyReportV1`. |
| Proof token issuer (`proof_token_service`) | Genera tokens opacos por accion de moderacion para que clientes verifiquen decisiones off-chain. | Usa tokens cegados + compromisos Merkle. |
| Storage y auditoria | RocksDB en gateways para enforcement activo; Postgres + object storage para historico. | Retencion hot 180 dias, archivo cold 7 anos. |

### Flujo de datos
1. El compliance controller obtiene denylists externas (BadBits, regionales, takedowns legales), normaliza a `DenylistEntryV1`.
2. El controller aplica la matriz de politicas, firma el update (`ComplianceUpdateV1`) con llave Dilithium3, publica via Torii + IPFS.
3. Los gateways se suscriben via WebSocket/REST, verifican firma, actualizan cache local RocksDB y confirman receipt.
4. Durante el manejo de requests, el interceptor de compliance verifica la evidencia `(manifest_cid, provider_id, account_id, url, ip, jurisdiction)` contra reglas activas + toggles del operador. Si corresponde bloqueo, retorna respuesta apropiada y emite proof token.
5. Todas las decisiones se registran localmente y se envian al transparency reporter; reportes semanales y compromisos Merkle se publican al Governance DAG.
6. Los operadores revisan dashboards, responden apelaciones (SFM-4b), ajustan toggles con entradas de auditoria.

## Ingesta de denylist
- **Fuentes** (extensible):
  - Feed canonico BadBits (`https://badbits.sora.net/v2/list.json`)
  - Feeds regionales de compliance (EU DSA, DMCA, etc.)
  - Acciones internas de governance (takedowns manuales).
  - Cuarentena de AI pre-screen (desde el servicio SFM-4a).
- **Schema** (`sorafs_manifest::compliance`):
  ```norito
  struct DenylistEntryV1 {
      list_id: String,
      entry_id: String,
      kind: DenylistKindV1,   // url | hash | cid | account | manifest | provider
      value: String,
      jurisdiction: Option<String>,
      issued_at: Timestamp,
      expires_at: Option<Timestamp>,
      reason_code: Option<String>,
      confidence: Option<u8>, // 0-100
      source_signature: Signature,
  }
  ```
- **Empaquetado de updates**:
  ```norito
  struct ComplianceUpdateV1 {
      update_id: Uuid,
      generated_at: Timestamp,
      entries: Vec<DenylistEntryV1>,
      removed_entries: Vec<String>, // entry_id
      policy_snapshot: CompliancePolicyV1,
      signature: Signature,
  }
  ```
- Updates publicados cada 10 minutos (incrementales) y snapshot diario completo pinneado en IPFS. Los gateways usan semantica `If-None-Match` con `update_id`.
- El controller mantiene un LRU de dedupe para evitar replay; historial de updates almacenado en Postgres (`compliance_updates` table).
- Gateways reconocen updates via Torii (`POST /compliance/update/ack`). Falta de ack -> alerta.
- **Tooling de operador:** `cargo xtask sorafs-gateway denylist pack --input <denylist.json> [--out <dir>] [--label <name>]` canoniza el JSON de denylist, calcula la raiz Merkle y escribe:
  - `<label>_<timestamp>.json` — bundle JSON listo para firmar (con entradas canonicas + pruebas).
  - `<label>_<timestamp>.to` — bundle codificado en Norito para ingestion Torii.
  - `<label>_<timestamp>_root.txt` — raiz Merkle + timestamp para adjuntar en auditoria.
  El empaquetador impone orden estable y produce pruebas de inclusion para que compliance adjunte artefactos a atestaciones GAR y reportes de transparencia sin scripts ad hoc.
- **Tracking de cambios:** `cargo xtask sorafs-gateway denylist diff --old <bundle.json> --new <bundle.json> [--report-json <path>]` compara dos bundles empaquetados, imprime entradas agregadas/eliminadas y opcionalmente emite un record JSON de evidencia. Los reviewers del Ministry adjuntan el output al packet MINFO-6 para que cada promocion de denylist tenga un rastro before/after determinista.
- **Verificacion de bundle:** `cargo xtask sorafs-gateway denylist verify --bundle <bundle.json> [--norito <bundle.to>] [--root <root.txt>] [--report-json <path>]` recomputa cada entrada canonica, prueba Merkle, payload Norito y `_root` antes de publicar. El comando falla rapido ante drift de digest/pruebas/metadata y puede emitir un receipt JSON para submissions GAR.

## Matriz de politicas y toggles de moderacion
- **Matriz de politicas** (`compliance_policy.toml`) mapea `list_id` a acciones:
  ```
  [lists.badbits]
  actions = ["block_content", "log_incident", "gar_notify"]
  severity = "critical"

  [lists.eu_dsa]
  actions = ["geo_block", "log_incident", "legal_notify"]
  jurisdictions = ["EU"]

  [lists.operator_manual]
  actions = ["block_content"]
  ```
- **Toggles de operador**:
  - API `POST /v2/moderation/toggle` acepta `ToggleRequestV1` (gateway_id, policy_id, enabled, expires_at, reason, operator_signature).
  - Requiere quorum: default 2-of-3 para produccion; toggles almacenados en `moderation_toggles` con audit record completo.
  - Gateways hacen polling de toggles; aplican la union de politica global + overrides por gateway.
- **Categorias**: tipos de contenido (illegal, copyright, sensitive), toggles de red (denylist-only, manual), toggles de feature (proof tokens on/off para testing).
- **Integracion de apelaciones**: toggles referencian `appeal_case_id` de SFM-4b cuando se aplican por decision del panel.

## Workflow de enforcement
- **Intercepcion de requests**:
  1. Para cada request/evento de streaming, el modulo de compliance computa un vector de evidencia: `(manifest_cid, provider_id, account_id, url, ip, jurisdiction)`.
  2. Consultar entradas activas en RocksDB usando indices por prefijo por tipo. Evaluar matriz de politicas + toggles.
  3. Si la accion incluye `geo_block`, examinar geo del request (MaxMind DB) y aplicar.
  4. En bloqueo: retornar HTTP 451 (Unavailable For Legal Reasons) o el status configurado. Incluir `Sora-Moderation-Token`.
- **Instrumentacion de response**:
  - Siempre setear `Sora-Compliance-Version` con update ID y version de politica.
  - Para contenido permitido pero marcado como sensible (via AI pre-screen warnings), opcionalmente agregar `Sora-Moderation-Warning`.
- **Proof tokens** (`ProofTokenV1`):
  ```norito
  struct ProofTokenV1 {
      token_id: Uuid,
      moderation_type: ModerationTypeV1,
      entry_ids: Vec<String>,
      issued_at: Timestamp,
      expires_at: Option<Timestamp>,
      blinded_digest: Digest32,  // HMAC over evidence + secret
      gateway_signature: Ed25519Signature,
  }
  ```
  - El gateway loguea el token y lo incluye en el header `Sora-Moderation-Token: base64url(...)`.
  - Transparency reporter agrega tokens; los clientes pueden presentar el token para apelacion/review sin revelar el recurso exacto.
  - La implementacion vive en `iroha_crypto::sorafs::proof_token` y emite el frame binario
    `SFGT` (`magic || version || flags || moderation || issued_at || expires_at?
    || token_id || entry_count || entries || blinded_digest || signature`). El helper limita listas a 32 entradas (<=255 B cada una), deriva `blinded_digest` via BLAKE3 con clave y dominio `sorafs.proof_token.digest.v1`, firma `SIGNING_DOMAIN || body` usando Ed25519 y expone `encode_base64()` para headers.【crates/iroha_crypto/src/sorafs/proof_token.rs:1】
  - Las claves de digest rotan mensualmente; solo auditores/Governance reciben el secreto de digest, mientras la clave publica Ed25519 se publica para verificacion de firmas sin revelar el payload de evidencia.
  - Gateways tambien emiten `Sora-Cache-Version` (espejado como `sora-cache-version` en fetchers cliente); las proofs se atan a `blake3(b"sorafs.policy.binding.v1" || cache_version)` para impedir replays con cache obsoleto. El helper de honey probe del orquestador (`sorafs_car::policy::run_honey_probe`) hace cumplir 451/`denylisted`, presencia de cache-version y verificacion de proof-token por provider antes de drills de adopcion.

## Reportes de transparencia
- Reporte semanal (`TransparencyReportV1`):
  ```norito
  struct TransparencyReportV1 {
      report_id: Uuid,
      period_start: Timestamp,
      period_end: Timestamp,
      gateway_id: String,
      summary: TransparencySummaryV1,
      stats: Vec<TransparencyMetricV1>,
      tokens_published: Vec<ProofTokenIndexV1>,
      signature: Signature,
  }
  ```
  - Las estadisticas incluyen conteos por lista, por jurisdiccion, resultados de apelaciones, tiempo promedio de respuesta.
  - Reportes publicados en Governance DAG y pinneados en IPFS; hasheados para evidencia anti-tamper.
- Reporte mensual agregado combinando todos los gateways + comentario legal.
- Dashboards consumen `TransparencyReportV1` y muestran metricas via Grafana.

## Observabilidad y alertas
- Metricas:
  - `sorafs_compliance_update_lag_seconds`
  - `sorafs_compliance_entries_total{list_id,action}`
  - `sorafs_compliance_hits_total{gateway_id,list_id,action}`
  - `sorafs_moderation_toggles_active{policy_id}`
  - `sorafs_proof_tokens_issued_total{gateway_id,type}`
  - `sorafs_compliance_update_errors_total`
- Logs: logs JSON por evento de enforcement (`compliance_event`) capturando request ID, gateway, entry_id, action, version de politica, token_id.
- Alertas:
  - Lag de update > 30 minutos (warning) / > 2 horas (critical).
  - Entrada de lista desconocida.
  - Backlog de aprobacion de toggles > 12 horas.
  - Tasa de fallas de emision de proof tokens > 1% (issues de firma).

## Seguridad y governance
- Updates firmados con Dilithium3; llaves en HSM. Los gateways rechazan updates sin firma o invalidos.
- TLS/mTLS para transporte de updates. Gateways verifican certificado + OCSP.
- Aprobaciones de toggles requieren multi-sig de governance; audit log hasheado y comprometido a Governance DAG diariamente.
- Proof tokens hasheados con secreto por gateway rotado mensualmente; tokens incluyen expiracion.
- Privacidad: los tokens de moderacion evitan PII; solo identificadores hasheados se almacenan.
- Disaster recovery: el controller mantiene los ultimos 30 updates en storage local; gateways hacen fallback al ultimo known-good con modo degradado.

## Estrategia de testing
- Tests unitarios para evaluacion de politicas, aplicacion de toggles, firmado de proof tokens.
- Integration tests con denylists simuladas, verificando propagacion de updates y enforcement.
- Chaos tests: drop de updates, entradas corruptas, verificar alertas y safe mode del gateway.
- Performance: asegurar que los checks de compliance agregan < 1 ms de latencia mediana.
- Security tests: intentos de forjado de firma, abuso de toggles, protecciones de inyeccion.
- Reporting tests: verificar que el reporte de transparencia coincide con eventos logueados y raiz Merkle.

## Plan de rollout
1. Implementar controller de compliance, schemas Norito y modulo gateway (feature-flagged).
2. Desplegar entorno staging con denylists sinteticas; correr tests de enforcement, verificacion de tokens, generacion de reportes de transparencia.
3. Integrar UI/API de operadores para toggles; completar workflow de auditoria.
4. Conectar servicio de AI pre-screen (SFM-4a) para alimentar listas de cuarentena; integrar pipeline de apelaciones (SFM-4b) para decisiones de override.
5. Rollout en produccion:
   - Stage 0: modo pasivo (log-only) para asegurar precision.
   - Stage 1: habilitar enforcement para BadBits + listas manuales; monitorear metricas.
   - Stage 2: habilitar listas regionales, proof tokens, reportes de transparencia; publicar primer reporte semanal.
   - Stage 3: enforcement automatico para listas criticas con gating de alertas.
6. Actualizar documentacion del operador (`docs/source/sorafs_gateway_operator_playbook.md`) y dashboards de transparencia.
7. Registrar completion en status/roadmap una vez metricas estables y auditorias de compliance pasen.

## Checklist de implementacion
- [x] Definir schemas de compliance, matriz de politicas y mecanismo de update.
- [x] Especificar workflow de enforcement, proof tokens y toggles.
- [x] Documentar reporting de transparencia y superficies de publicacion.
- [x] Capturar metricas, alertas y requisitos de seguridad.
- [x] Detallar estrategia de testing y secuencia de rollout.
- [x] Anotar puntos de integracion con AI pre-screen, apelaciones y dashboards.

Con esta especificacion, los equipos pueden implementar la capa de compliance,
moderacion y transparencia necesaria para satisfacer requisitos legales y
mantener la confianza de la comunidad en la red de gateways SoraFS.
