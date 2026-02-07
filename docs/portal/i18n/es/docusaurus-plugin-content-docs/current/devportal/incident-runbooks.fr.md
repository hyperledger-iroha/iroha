---
lang: es
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbooks de incidentes y ejercicios de reversión

## Objetivo

El elemento de la hoja de ruta **DOCS-9** requiere guías prácticas además de un plan de repetición para que
les operatorurs du portail puissent recuperer des echecs de livraison sans deviner. esta nota
incidentes de couvre trois en una señal de fuerte: tasas de implementación, degradación de replicación y
Paneles de análisis: documente los ejercicios trimestrales que proporcionan la reversión de alias.
y la validación sintética funciona todos los días de combate en combate.

### Conexión de material

- [`devportal/deploy-guide`](./deploy-guide) - flujo de trabajo de empaquetado, firma y promoción de alias.
- [`devportal/observability`](./observability) - etiquetas de lanzamiento, análisis y referencias de sondas ci-dessous.
- `docs/source/sorafs_node_client_protocol.md`
  y [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - Telemetrie du registre et seuils d'escalade.
- `docs/portal/scripts/sorafs-pin-release.sh` y ayudantes `npm run probe:*`
  referencias en las listas de verificación.

### Telemetría y piezas de herramientas| Señal / Herramientas | Objetivo |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (cumplido/incumplido/pendiente) | Detecte los bloqueos de replicación y las infracciones de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Cuantifique la profundidad del trabajo pendiente y la latencia de finalización para la clasificación. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Montre les echecs cote gateway qui suivent souvent una tasa de implementación. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Sondas sintéticas que detectan las liberaciones y validan las reversiones. |
| `npm run check:links` | Casos de puerta de embargo; utilizar la mitigación después del chaque. |
| `sorafs_cli manifest submit ... --alias-*` (envuelto por `scripts/sorafs-pin-release.sh`) | Mecanismo de promoción/reversión de alias. |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Agregar los rechazos de telemetría/alias/TLS/replicación. Las alertas PagerDuty hacen referencia a estos paneles como antes. |

## Runbook: tasa de implementación o defectos de artefactos

### Condiciones de declinación

- Vista previa de las sondas/echouent de producción (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana en `torii_sorafs_gateway_refusals_total` o
  `torii_sorafs_manifest_submit_total{status="error"}` después del lanzamiento.
- QA manuel remarque des route cassees ou des pannes du proxy Pruébelo inmediatamente después
  la promoción de l'alias.

### Confinamiento inmediato1. **Generar implementaciones:** marcar la tubería CI con `DEPLOY_FREEZE=1` (entrada del flujo de trabajo
   GitHub) o poner en pausa el trabajo de Jenkins para que no haya ningún artefacto en la parte.
2. **Capturador de artefactos:** telecargador `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, et la sortie des probes du build en echec afin que
   la referencia de reversión de los resúmenes exactos.
3. **Notificador de las partes embarazadas:** almacenamiento SRE, plomo Docs/DevRel y el oficial de guardia
   Gobernanza para la concientización (surtout si `docs.sora` est impacte).

### Procedimiento de reversión

1. Identificador del último bien conocido del manifiesto (LKG). El flujo de trabajo de producción de las existencias
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Re-lier l'alias a ce manifest avec le helper de Shipping:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Registre le resume du rollback dans le ticket d'incident avec les digests du
   manifest LKG et du manifest en echec.

### Validación

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` y `sorafs_cli proof verify ...`
   (Vea la guía de implementación) para confirmar que el manifiesto repromu corresponde
   toujours au CAR archivo.
4. `npm run probe:tryit-proxy` para asegurar que el proxy Try-It staging esté rentable.

### Post-incidente1. Reactiver le pipeline de deploiement only después de haber comprendido la causa racine.
2. Mettre a jour les entrees "Lecciones aprendidas" en [`devportal/deploy-guide`](./deploy-guide)
   avec de nouveaux points, si besoin.
3. Buscar defectos para el conjunto de pruebas en echec (sonda, verificador de enlaces, etc.).

## Runbook - Degradación de replicación

### Condiciones de declinación

- Alerta: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  abrazadera_min(suma(torii_sorafs_replication_sla_total{resultado=~"cumplido|perdido"}), 1) <
  Colgante de 0,95` 10 minutos.
- `torii_sorafs_replication_backlog_total > 10` colgante 10 minutos (voir
  `pin-registry-ops.md`).
- Gobernanza señala una disponibilidad de alias lente después de la liberación.

### Triaje

1. Inspeccione los paneles [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) para
   Confirme si el trabajo pendiente está localizado en una clase de almacenamiento o en una flota de proveedores.
2. Croiser les logs Torii pour les advertencias `sorafs_registry::submit_manifest` afin de
   determinante si les envíos echouent.
3. Echantillonner la sante des replicas vía `sorafs_cli manifest status --manifest ...`
   (Liste los resultados por proveedor).

### Mitigación1. Reemettre le manifest avec un nombre de replicas plus once (`--pin-min-replicas 7`) vía
   `scripts/sorafs-pin-release.sh` afin que le planificador etale la cargo sur plus de los proveedores.
   Registre el nuevo resumen en el registro del incidente.
2. Si el trabajo pendiente es un proveedor único, lo desactivará temporalmente a través del programador
   de replicación (documento en `pin-registry-ops.md`) y soumettre un nouveau manifest
   Forcant les otros proveedores a rafraichir l'alias.
3. Quand la fraicheur de l'alias est plus critique que la parite de replication, re-lier
   l'alias a un manifest chaud deja stage (`docs-preview`), después publier un manifest de
   Después de una vez que SRE a nettoye le backlog.

### Recuperación y cierre

1. Vigilante `torii_sorafs_replication_sla_total{outcome="missed"}` para asegurar que le
   compteur se estabiliza.
2. Capturer la salida `sorafs_cli manifest status` como antes de que cada réplica esté
   volver en conformidad.
3. Revisar o medir un día el post-mortem del trabajo pendiente de replicación con las cadenas
   etapas (proveedores de escalado, tuning du fragmentador, etc.).

## Runbook: panel de análisis o telemetría

### Condiciones de declinación

- `npm run probe:portal` reutilice los paneles de control antes de recibir eventos
  `AnalyticsTracker` colgante >15 minutos.
- La revisión de privacidad detecta una casa desatendida de eventos abandonados.
- `npm run probe:tryit-proxy` repite las rutas `/probe/analytics`.

### Respuesta1. Verificador de entradas de compilación: `DOCS_ANALYTICS_ENDPOINT` y
   `DOCS_ANALYTICS_SAMPLE_RATE` en el artefacto de liberación (`build/release.json`).
2. Reejecutor `npm run probe:portal` con `DOCS_ANALYTICS_ENDPOINT` apuntando a la versión le
   Collector de staging pour confirmer que le tracker emet encore des payloads.
3. Si los coleccionistas están caídos, defina `DOCS_ANALYTICS_ENDPOINT=""` y reconstruya para
   que le tracker circuito judicial; consignador la fenetre d'outage dans la timeline.
4. Validador que `scripts/check-links.mjs` continúa con la huella digital `checksums.sha256`
   (Los paneles de análisis no deben *pas* bloquear la validación del mapa del sitio).
5. Una vez que el coleccionista retabli, lancer `npm run test:widgets` para ejecutar archivos
   pruebas unitarias del análisis auxiliar antes de volver a publicar.

### Post-incidente

1. Mettre a jour [`devportal/observability`](./observability) con las nuevas limitaciones
   du coleccionista o les exigencias de muestreo.
2. Emettre une Notice Governance Sides Donnees Analytics Ont ete Perdues ou Redactees
   fuera de la política.

## Ejercicios trimestriels de resiliencia

Lancer les deux drills durant le **premier mardi de chaque trimestre** (enero/abril/julio/octubre)
O inmediatamente después de cualquier cambio mayor de infraestructura. Stocker les artefactos sous
`artifacts/devportal/drills/<YYYYMMDD>/`.| Taladro | Etapas | Preuve |
| ----- | ----- | -------- |
| Repetición de la reversión de alias | 1. Rejuvenece la reversión "Tasa de implementación" con el manifiesto de producción más reciente.2. Re-lier a production une fois que les probes passent.3. Registre `portal.manifest.submit.summary.json` y los registros de sondas en el expediente del taladro. | `rollback.submit.json`, salida de sondas y liberación de etiqueta de repetición. |
| Auditoría de validación sintética | 1. Lancer `npm run probe:portal` et `npm run probe:tryit-proxy` contra producción y puesta en escena.2. Lancer `npm run check:links` y archivador `build/link-report.json`.3. Se unen capturas de pantalla/exportaciones de paneles Grafana que confirman el éxito de las sondas. | Registros de sondas + `link-report.json` referentes a la huella digital del manifiesto. |

Escalader les drills manques au manager Docs/DevRel et a la revue Governance SRE, car le
La hoja de ruta exige un preuve trimestrielle deterministe que le rollback d'alias et les probes
portal restent sains.

## Coordinación PagerDuty y guardia- El servicio PagerDuty **Docs Portal Publishing** posee las alertas generadas desde entonces
  `dashboards/grafana/docs_portal.json`. Las reglas `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` y `DocsPortal/TLSExpiry` página principal Docs/DevRel
  con Storage SRE en secondaire.
- Cuando en esta página, incluya el `DOCS_RELEASE_TAG`, junto con las capturas de pantalla de los paneles
  Grafana impacta y lier la sortie probe/link-check dans les notes d'incident avant
  de comenzar la mitigación.
- Mitigación después (revertir o volver a implementar), reejecutor `npm run probe:portal`,
  `npm run check:links`, y capturador de instantáneas Grafana montando las medidas
  ingresos dans les seuils. Únase a todas las pruebas del incidente PagerDuty
  resolución de vanguardia.
- Si dos alertas se declencent en meme temps (por ejemplo, vencimiento de TLS más trabajo pendiente), prueba
  negativas en primer ministro (arreter la publicación), ejecutor la procedimiento de reversión, puis
  Traer TLS/backlog con Storage SRE en el puente.