---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Revista de seguridad SF-6
resumen: Constataciones y acciones de seguimiento de la evaluación independiente de la firma sin clave, del streaming de pruebas y de los canales de envío de manifiestos.
---

# Revista de seguridad SF-6

**Fenêtre de evaluación :** 2026-02-10 → 2026-02-18  
**Líderes de revista:** Gremio de ingeniería de seguridad (`@sec-eng`), Grupo de trabajo de herramientas (`@tooling-wg`)  
**Perímetro:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), API de streaming de prueba, gestión de manifiestos en Torii, integración Sigstore/OIDC, ganchos de liberación CI.  
**Artefactos:**  
- Fuente CLI y pruebas (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manifiesto/prueba de manejadores Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Liberación de automatización (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Arnés de paridad determinada (`crates/sorafs_car/tests/sorafs_cli.rs`, [Rapport de parité GA SoraFS Orchestrator](./orchestrator-ga-parity.md))

## Metodología1. **Ateliers de modelado de amenazas** cartografían las capacidades de ataque para las publicaciones de desarrolladores, los sistemas CI y los nuevos Torii.  
2. **Revisión de código** para activar las superficies de identificación (cambio de tokens OIDC, firma sin clave), validación de manifiestos Norito y contrapresión del streaming de prueba.  
3. **Las pruebas dinámicas** activan los manifiestos de accesorios y simulan los modos de panel (repetición de tokens, manipulación de manifiestos, secuencias de prueba tronqués) a través del arnés de paridad y de las unidades fuzz dédiés.  
4. **Inspección de configuración** para validar los valores predeterminados `iroha_config`, la gestión de las banderas CLI y los scripts de liberación para garantizar las ejecuciones determinadas y auditables.  
5. **Entretien de Processus** a confirmé le flux de remédiation, les chemins d'escalade et la captura de evidencia de auditoría con los propietarios de liberación de Tooling WG.

## Resumen de estadísticas| identificación | Sévérite | Zona | constante | Resolución |
|----|----------|------|---------|------------|
| SF6-SR-01 | Élevée | Firma sin clé | Los valores predeterminados de audiencia de los tokens OIDC están implícitos en las plantillas CI, con el riesgo de reproducción entre inquilinos. | Hay una exigencia explícita de `--identity-token-audience` en los ganchos de lanzamiento y plantillas CI ([proceso de lanzamiento](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI échoue désormais si l'audience est omise. |
| SF6-SR-02 | Moyenne | Transmisión de prueba | Los tubos de contrapresión aceptan buffers de suscriptores sin límite, lo que permite el almacenamiento de memoria. | `sorafs_cli proof stream` impone las colas de canales nacidos con un truncamiento determinado, publica los currículums Norito y aborta la transmisión; El espejo Torii se ha actualizado para llevar los fragmentos de respuesta (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Moyenne | Envío de manifiestos | La CLI acepta manifiestos sin verificar que los planes fragmentados se embarquen cuando `--plan` esté ausente. | `sorafs_cli manifest submit` recalcula y compara los resúmenes de CAR o si `--expect-plan-digest` está disponible, rechazando las discrepancias y exponiendo las sugerencias de solución. Des tests couvrent succès/échecs (`crates/sorafs_car/tests/sorafs_cli.rs`). || SF6-SR-04 | Faible | Pista de auditoría | La lista de verificación de liberación no dispone de un registro de aprobación firmado para la revista de seguridad. | Una sección adicional [proceso de lanzamiento] (../developer-releases.md) exige el adjunto de los hashes del memo de revista y la URL del ticket de cierre antes de GA. |

Todas las constantes alta/media están corregidas durante la ventana de revisión y validadas por el arnés de paridad existente. Aucun cuestión crítica latente ne reste.

## Validación de controles- **Portée des identifiants:** Les templates CI imponent désormais audiencia y emisor explícitos; La CLI y el asistente de liberación suenan rápidamente si `--identity-token-audience` no acompañan a `--identity-token-provider`.  
- **Repetición determinante:** Las pruebas del día contienen flujos positivos/negativos de envío de manifiestos, lo que garantiza que los resúmenes en desajuste resten des échecs no determinados y son señales antes de tocar la respuesta.  
- **Transmisión a prueba de contrapresión:** Torii desorma los elementos PoR/PoTR a través de los canales nacidos, y la CLI no conserva los cantos de latencia tronqués + cinco ejemplos de control, evitando el croissance sin límite en función de los currículums determinados.  
- **Observabilité:** Los compteursproof streaming (`torii_sorafs_proof_stream_*`) y los currículums CLI capturan las razones del aborto, ofreciendo las migas de pan de auditoría a los operadores.  
- **Documentación:** Las guías de desarrolladores ([índice de desarrollador](../developer-index.md), [referencia CLI](../developer-cli.md)) señalan las banderas sensibles y los flujos de trabajo de escalada.

## Agregar a la lista de verificación de liberación

Les release managers **doivent** se unen a los preuves suivantes lors de la promoción de un candidato GA:1. Hash du memo de revue de sécurité le plus récent (este documento).  
2. Lien vers le ticket de remédiation suivi (ej. `governance/tickets/SF6-SR-2026.md`).  
3. Salida de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` montando los argumentos audiencia/emisor explícitos.  
4. Registros capturados del arnés de parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmación de que las notas de la versión Torii incluyen los ordenadores de télémétrie de streaming de prueba transmitidos.

No recolectes los artefactos ci-dessus bloque le sign-off GA.

**Hashes de artefactos de referencia (aprobación 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Suivis en attente

- **Mise à jour du modelo de amenaza:** Répéter esta revista cada trimestre o antes de los ajouts majeurs de flags CLI.  
- **Fusing de cobertura:** Las codificaciones de transmisión de prueba de transporte son difusas a través de `fuzz/proof_stream_transport`, identidad couvrant, gzip, deflate y zstd.  
- **Repetición del incidente:** Planifique un ejercicio de operación que simule un compromiso de token y una reversión de manifiesto, para asegurarse de que la documentación refleje los procedimientos realizados.

## Aprobación

- Representante del Gremio de Ingeniería de Seguridad: @sec-eng (2026-02-20)  
- Grupo de trabajo de herramientas representativo: @tooling-wg (2026-02-20)

Conservar las aprobaciones firmadas con el paquete de artefactos de liberación.