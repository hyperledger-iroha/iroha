---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Revisión de seguridad SF-6
resumen: Hallazgos y tareas de seguimiento de la evaluación independiente de keyless signing,proof streaming y pipelines de envío de manifests.
---

# Revisión de seguridad SF-6

**Ventana de evaluación:** 2026-02-10 -> 2026-02-18  
**Líderes de revisión:** Gremio de ingeniería de seguridad (`@sec-eng`), Grupo de trabajo de herramientas (`@tooling-wg`)  
**Alcance:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), APIs de prueba de streaming, manejo de manifiestos en Torii, integración Sigstore/OIDC, ganchos de liberación en CI.  
**Artefactos:**  
- Fuente del CLI y pruebas (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manejadores de manifiesto/prueba en Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Automatización de liberación (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Arnés de paridad determinista (`crates/sorafs_car/tests/sorafs_cli.rs`, [Reporte de Paridad GA del Orchestrator SoraFS](./orchestrator-ga-parity.md))

## Metodología1. **Talleres de modelado de amenazas** mapearon capacidades de atacantes para estaciones de trabajo de desarrolladores, sistemas CI y nodos Torii.  
2. **Revisión de código** enfoco superficies de credenciales (intercambio de tokens OIDC, firma sin llave), validación de manifiestos Norito y contrapresión en streaming de prueba.  
3. **Prueba dinámica** reprodujo manifiestos de accesorios y simulo modos de falla (repetición de tokens, manipulación de manifiestos, flujos de prueba truncados) usando el arnés de paridad y fuzz drives a medida.  
4. **Inspección de configuración** valido defaults de `iroha_config`, manejo de flags del CLI y scripts de liberación para asegurar ejecuciones deterministas y auditables.  
5. **Entrevista de proceso** confirmo el flujo de remediación, rutas de escalada y captura de evidencia de auditoría con los propietarios de liberación de Tooling WG.

## Resumen de hallazgos| identificación | Severidad | Área | Hallazgo | Resolución |
|----|----------|------|---------|------------|
| SF6-SR-01 | Alta | Firma sin llave | Los defaults de audiencia del token OIDC estaban implícitos en plantillas de CI, con riesgo de repetición entre inquilinos. | Se agrega la aplicación explícita de `--identity-token-audience` en ganchos de liberación y plantillas de CI ([proceso de liberación](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI ahora falla si se omite la audiencia. |
| SF6-SR-02 | Medios | Transmisión de prueba | Los caminos de contrapresión aceptaban buffers de suscriptores sin límite, habilitando agotamiento de memoria. | `sorafs_cli proof stream` impone tamanos de canal acotados con truncamiento determinista, registrando resúmenes Norito y abortando el stream; el espejo Torii se actualiza para acotar trozos de respuesta (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Medios | Envío de manifiestos | El CLI aceptaba manifests sin verificar planos de trozos embebidos cuando `--plan` estaba ausente. | `sorafs_cli manifest submit` ahora recomputa y compara resúmenes de CAR salvo que se prueba `--expect-plan-digest`, rechazando discrepancias y mostrando pistas de remediación. Los tests cubren casos de éxito/falla (`crates/sorafs_car/tests/sorafs_cli.rs`). || SF6-SR-04 | Baja | Pista de auditoría | El checklist de liberación carecia de un registro de aprobación firmado para la revisión de seguridad. | Se agrega una sección en [proceso de liberación](../developer-releases.md) que requiere adjuntar hashes del memo de revisión y URL del ticket de cierre de sesión antes de GA. |

Todos los hallazgos alto/medio se corrigieron durante la ventana de revisión y se validaron con el arnés de paridad existente. No quedan temas críticos latentes.

## Validación de controles- **Alcance de credenciales:** Los templates de CI ahora exigen audiencia y expedidor explícitos; El CLI y el ayudante de liberación fallan rápidamente salvo que `--identity-token-audience` acompaña a `--identity-token-provider`.  
- **Replay determinista:** Tests actualizados cubren flujos positivos/negativos de envío de manifests, asegurando que digest desalineados sigan siendo fallas no deterministas y se detectan antes de tocar la red.  
- **Back-pression enproof streaming:** Torii ahora transmite items PoR/PoTR sobre canales acotados, y el CLI retiene solo muestras truncadas de latencia + cinco ejemplos de falla, evitando el crecimiento sin límite y manteniendo resúmenes deterministas.  
- **Observabilidad:** Contadores de prueba streaming (`torii_sorafs_proof_stream_*`) y resúmenes del CLI capturan razones de aborto, entregando pan rallado de auditoria a operadores.  
- **Documentación:** Guías para desarrolladores ([índice de desarrollador](../developer-index.md), [referencia CLI](../developer-cli.md)) indican banderas sensibles a seguridad y flujos de trabajo de escalamiento.

## Adiciones al checklist de liberación

Los release managers **deben** adjuntar la siguiente evidencia al promover un candidato de GA:1. Hash del memo más reciente de revisión de seguridad (este documento).  
2. Link al ticket de remediación seguido (por ejemplo, `governance/tickets/SF6-SR-2026.md`).  
3. Salida de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` mostrando argumentos explícitos de audiencia/emisor.  
4. Troncos capturados del arnés de paridad (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmación de que las notas de la versión de Torii incluyen contadores de telemetría de prueba de streaming acotado.

No recolectar los artefactos anteriores bloquea el sign-off de GA.

**Hashes de artefactos de referencia (aprobación 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Seguimientos pendientes

- **Actualización del modelo de amenaza:** Repetir esta revisión trimestralmente o antes de grandes adiciones de flags del CLI.  
- **Cobertura de fuzzing:** Los codificaciones de transporte de prueba streaming se fuzzearon vía `fuzz/proof_stream_transport`, cubriendo payloads Identity, gzip, deflate y zstd.  
- **Ensayo de incidentes:** Programar un ejercicio de operadores que simula compromiso de token y reversión de manifiesto, garantizando que la documentación refleja los procedimientos practicados.

## Aprobación

- Representante de Security Engineering Guild: @sec-eng (2026-02-20)  
- Representante de Tooling Working Group: @tooling-wg (2026-02-20)

Almacena las aprobaciones firmadas junto al paquete de artefactos de liberación.