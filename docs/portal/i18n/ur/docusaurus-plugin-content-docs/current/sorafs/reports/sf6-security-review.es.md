---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/sf6-security-review.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Revision de seguridad SF-6
summary: Hallazgos y tareas de seguimiento de la evaluacion independiente de keyless signing, proof streaming y pipelines de envio de manifests.
---

# Revision de seguridad SF-6

**Ventana de evaluacion:** 2026-02-10 -> 2026-02-18  
**Leads de revision:** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**Alcance:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), APIs de proof streaming, manejo de manifests en Torii, integracion Sigstore/OIDC, hooks de release en CI.  
**Artefactos:**  
- Fuente del CLI y tests (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Handlers de manifest/proof en Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Automatizacion de release (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Harness de paridad determinista (`crates/sorafs_car/tests/sorafs_cli.rs`, [Reporte de Paridad GA del Orchestrator SoraFS](./orchestrator-ga-parity.md))

## Metodologia

1. **Workshops de threat modeling** mapearon capacidades de atacantes para estaciones de trabajo de developers, sistemas CI y nodos Torii.  
2. **Code review** enfoco superficies de credenciales (intercambio de tokens OIDC, keyless signing), validacion de manifests Norito y back-pressure en proof streaming.  
3. **Testing dinamico** reprodujo manifests de fixtures y simulo modos de falla (token replay, manifest tampering, proof streams truncados) usando el harness de paridad y fuzz drives a medida.  
4. **Inspeccion de configuracion** valido defaults de `iroha_config`, manejo de flags del CLI y scripts de release para asegurar ejecuciones deterministas y auditables.  
5. **Entrevista de proceso** confirmo el flujo de remediacion, rutas de escalamiento y captura de evidencia de auditoria con los owners de release de Tooling WG.

## Resumen de hallazgos

| ID | Severidad | Area | Hallazgo | Resolucion |
|----|----------|------|---------|------------|
| SF6-SR-01 | Alta | Keyless signing | Los defaults de audiencia del token OIDC eran implicitos en templates de CI, con riesgo de replay entre tenants. | Se agrego la aplicacion explicita de `--identity-token-audience` en hooks de release y templates de CI ([release process](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI ahora falla si se omite la audiencia. |
| SF6-SR-02 | Media | Proof streaming | Los caminos de back-pressure aceptaban buffers de suscriptores sin limite, habilitando agotamiento de memoria. | `sorafs_cli proof stream` impone tamanos de canal acotados con truncamiento determinista, registrando resumenes Norito y abortando el stream; el espejo Torii se actualizo para acotar chunks de respuesta (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Media | Envio de manifests | El CLI aceptaba manifests sin verificar planes de chunks embebidos cuando `--plan` estaba ausente. | `sorafs_cli manifest submit` ahora recomputa y compara digests de CAR salvo que se provea `--expect-plan-digest`, rechazando mismatches y mostrando pistas de remediacion. Los tests cubren casos de exito/falla (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Baja | Audit trail | El checklist de release carecia de un log de aprobacion firmado para la revision de seguridad. | Se agrego una seccion en [release process](../developer-releases.md) que requiere adjuntar hashes del memo de revision y URL del ticket de sign-off antes de GA. |

Todos los hallazgos high/medium se corrigieron durante la ventana de revision y se validaron con el harness de paridad existente. No quedan issues criticos latentes.

## Validacion de controles

- **Alcance de credenciales:** Los templates de CI ahora exigen audiencia y issuer explicitos; el CLI y el helper de release fallan rapido salvo que `--identity-token-audience` acompane a `--identity-token-provider`.  
- **Replay determinista:** Tests actualizados cubren flujos positivos/negativos de envio de manifests, asegurando que digests desalineados sigan siendo fallas no deterministas y se detecten antes de tocar la red.  
- **Back-pressure en proof streaming:** Torii ahora transmite items PoR/PoTR sobre canales acotados, y el CLI retiene solo muestras truncadas de latencia + cinco ejemplos de falla, evitando crecimiento sin limite y manteniendo resumenes deterministas.  
- **Observabilidad:** Contadores de proof streaming (`torii_sorafs_proof_stream_*`) y resumenes del CLI capturan razones de aborto, entregando breadcrumbs de auditoria a operadores.  
- **Documentacion:** Guías para developers ([developer index](../developer-index.md), [CLI reference](../developer-cli.md)) indican flags sensibles a seguridad y workflows de escalamiento.

## Adiciones al checklist de release

Los release managers **deben** adjuntar la siguiente evidencia al promover un GA candidate:

1. Hash del memo mas reciente de revision de seguridad (este documento).  
2. Link al ticket de remediacion seguido (por ejemplo, `governance/tickets/SF6-SR-2026.md`).  
3. Output de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` mostrando argumentos explicitos de audiencia/issuer.  
4. Logs capturados del harness de paridad (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmacion de que las release notes de Torii incluyen contadores de telemetria de proof streaming acotado.

No recolectar los artefactos anteriores bloquea el sign-off de GA.

**Hashes de artefactos de referencia (sign-off 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Seguimientos pendientes

- **Actualizacion del threat model:** Repetir esta revision trimestralmente o antes de grandes adiciones de flags del CLI.  
- **Cobertura de fuzzing:** Los encodings de transporte de proof streaming se fuzzearon via `fuzz/proof_stream_transport`, cubriendo payloads identity, gzip, deflate y zstd.  
- **Ensayo de incidentes:** Programar un ejercicio de operadores que simule token compromise y rollback de manifest, garantizando que la documentacion refleje procedimientos practicados.

## Aprobacion

- Representante de Security Engineering Guild: @sec-eng (2026-02-20)  
- Representante de Tooling Working Group: @tooling-wg (2026-02-20)

Almacena las aprobaciones firmadas junto al bundle de artefactos de release.
