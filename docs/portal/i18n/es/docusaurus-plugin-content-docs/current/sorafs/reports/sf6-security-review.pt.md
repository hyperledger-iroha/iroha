---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Revisao de seguranca SF-6
resumen: Achados e itens de acompanhamento da avaliacao independientee de firma sin llave, transmisión de pruebas y canalizaciones de envío de manifiestos.
---

# Revisao de seguranca SF-6

**Janela de avaliacao:** 2026-02-10 -> 2026-02-18  
**Líderes de revisión:** Gremio de ingeniería de seguridad (`@sec-eng`), Grupo de trabajo de herramientas (`@tooling-wg`)  
**Escopo:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), API de transmisión de prueba, manejo de manifiestos Torii, integración Sigstore/OIDC, ganchos de liberación CI.  
**Artefactos:**  
- Fuente de CLI y pruebas (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manejadores de manifiesto/prueba Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Liberación de automatización (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Arnés de paridad determinista (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS Informe de paridad GA de Orchestrator](./orchestrator-ga-parity.md))

## Metodología1. **Talleres de modelado de amenazas** mapear capacidades de ataque para estaciones de trabajo de desarrolladores, sistemas CI y nodos Torii.  
2. **Revisión de código** se centra en superficies de credenciales (intercambio de tokens OIDC, firma sin llave), validación de manifiestos Norito y contrapresión en transmisión de pruebas.  
3. **Pruebas dinámicas** reejecutar manifiestos de dispositivos y modos de falla simultáneos (repetición de tokens, manipulación de manifiestos, flujos de prueba truncados) usando arnés de paridad y unidades fuzz en medida.  
4. **Inspección de configuración** valida los valores predeterminados `iroha_config`, manejo de indicadores CLI y scripts de lanzamiento para garantizar ejecuciones determinísticas y auditadas.  
5. **Entrevista del proceso** confirme el flujo de remediación, las rutas de escalamiento y la captura de evidencia de auditoría con los propietarios de la versión en Tooling WG.

## Resumen de hallazgos| identificación | Gravedad | Área | Encontrar | Resolución |
|----|----------|------|---------|------------|
| SF6-SR-01 | Alto | Firma sin llave | Los valores predeterminados de la audiencia del token OIDC están implícitos en plantillas de CI, con el riesgo de repetición entre inquilinos. | Esta aplicación adicional explícita de `--identity-token-audience` en ganchos de liberación y plantillas de CI ([proceso de liberación](../developer-releases.md), `docs/examples/sorafs_ci.md`). O CI agora falha quando a audiencia e omitida. |
| SF6-SR-02 | Medio | Transmisión de prueba | Las rutas de contrapresión aceitan buffers de suscriptores sin límites, lo que permite el agotamiento de la memoria. | `sorafs_cli proof stream` aplica tamaños de canal limitados con truncamiento determinístico, registra resúmenes Norito y cancela la transmisión; o Torii mirror foi actualizado para limitar fragmentos de respuesta (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Medio | Presentación manifiesta | O CLI aceitava manifiesta sin verificar planes de fragmentos incrustados cuando `--plan` estaba ausente. | `sorafs_cli manifest submit` ahora recomputa y compara CAR digiere a menos que `--expect-plan-digest` seja fornecido, rejeitando discrepancias y exibindo sugerencias de remediación. Tests cobrem casos de éxito/falha (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Bajo | Pista de auditoría | La lista de verificación de liberación es un registro de aprobación firmado para una revisión de seguridad. | Foi adicionada uma secao em [proceso de liberación](../developer-releases.md) exigiendo hashes anexar para revisar la nota y la URL para el ticket de cierre de sesión antes de GA. |Todos los achados foram high/medium corrigidos durante a janela de revisao e validados pelo parity Harness existente. Nenhum issues critico latente permanece.

## Validación de controles

- **Alcance de la credencial:** Plantillas de CI para exigem audiencia y emisor explícitos; o CLI y o release helper falham rapido a menos que `--identity-token-audience` acompaña a `--identity-token-provider`.  
- **Repetición determinista:** Pruebas atualizadas cobrem flujos positivos/negativos de presentación manifiesta, garantizando que los resúmenes no coincidentes continúan sendo falhas nao determinísticas y sejam expostas antes de tocar a rede.  
- **Prueba de contrapresión de transmisión:** Torii ahora hace frente a la transmisión de elementos PoR/PoTR en canales limitados, y el CLI reten apenas muestras de latencia truncadas + cinco ejemplos de falla, prevenindo crescimento sem limite y mantendo resúmenes determinísticos.  
- **Observabilidad:** Contadores de transmisión de prueba (`torii_sorafs_proof_stream_*`) y resúmenes CLI capturan motivos de cancelación, lo que requiere auditoría de rutas de navegación para los operadores.  
- **Documentación:** Guías para desarrolladores ([índice de desarrollador](../developer-index.md), [referencia CLI](../developer-cli.md)) destacan banderas sensibles a flujos de trabajo de seguridad y escalada.

## Adiciones a la lista de verificación de lanzamiento

Los administradores de versiones **devem** anexar la siguiente evidencia para promover un candidato de GA:1. Hash hacer un memorándum de revisión de seguridad más reciente (este documento).  
2. Enlace para el seguimiento del ticket de remediación (ej.: `governance/tickets/SF6-SR-2026.md`).  
3. Salida de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` mostrando argumentos audiencia/emisor explícitos.  
4. Troncos capturados do arnés de paridad (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirme que las notas de la versión Torii incluyen contadores de telemetría de transmisión de prueba limitada.

Nao coletar os artefactos acima bloqueia o sign-off de GA.

**Hashes de artefactos de referencia (aprobación 2026-02-20):**

- `sf6_security_review.md` - `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Seguimientos destacados

- **Actualización del modelo de amenazas:** Repita esta revisión trimestral o antes de los grandes indicadores de CLI.  
- **Cobertura difusa:** Codificaciones de transporte de transmisión de prueba difusas mediante `fuzz/proof_stream_transport`, identidad de carga útil cobrindo, gzip, deflate y zstd.  
- **Ensayo de incidente:** Agenda un ejercicio de operadores simulando compromiso de token y reversión de manifiesto, garantizando que a documentacao reflita procedimentos praticados.

## Aprobación

- Representante del Gremio de Ingeniería de Seguridad: @sec-eng (2026-02-20)  
- Representante del Grupo de Trabajo sobre Herramientas: @tooling-wg (2026-02-20)

Guarde aprueba assinados junto al paquete de artefactos de lanzamiento.