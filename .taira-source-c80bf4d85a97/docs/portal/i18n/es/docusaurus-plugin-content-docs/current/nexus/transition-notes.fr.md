---
lang: es
direction: ltr
source: docs/portal/docs/nexus/transition-notes.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas-de-transición-nexus
título: Notas de transición de Nexus
descripción: Miroir de `docs/source/nexus_transition_notes.md`, couvrant les preuves de Transition Phase B, le calendrier d'audit et les mitigations.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notas de transición de Nexus

Ce journal suit le travail restant de **Phase B - Nexus Transition Foundations** justo al final de la checklist de lancement multi-lane. Complete las entradas de hitos en `roadmap.md` y garde les preuves references par B1-B4 en un solo lugar a fin de que la gobernanza, SRE y les leads SDK participen en el meme source de verite.

## Portée et cadence

- Couvre les audits routed-trace et les guardrails de telemetrie (B1/B2), el conjunto de deltas de configuración aprobados por la gobernanza (B3) y les suivis de repetition de lancement multi-lane (B4).
- Reemplace la nota temporal de cadencia que viva aquí; Después de la auditoría del primer trimestre de 2026, la relación detallada se encuentra en `docs/source/nexus_routed_trace_audit_report_2026q1.md`, y esta página mantiene el calendario actual y el registro de mitigaciones.
- Mettez a jour les tables apres chaque fenetre routed-trace, vote de gouvernance ou repetition de lancement. Cuando los artefactos aparecen, refletez el nuevo emplazamiento en esta página para que los documentos disponibles (estado, paneles, portales SDK) puedan encontrarse en un soporte estable.

## Instantánea de los años anteriores (2026 Q1-Q2)| Flujo de trabajo | Preuves | Propietario(s) | Estatuto | Notas |
|------------|----------|----------|--------|-------|
| **B1 - Auditorías de seguimiento enrutado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @operaciones de telemetría, @gobernanza | Completado (primer trimestre de 2026) | Trois fenetres d'audit registradores; El retardo TLS de `TRACE-CONFIG-DELTA` está cerrado mientras se vuelve a ejecutar Q2. |
| **B2 - Remediación de telemetría y barandillas** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Completo | Paquete de alerta, política del bot de diferenciación y cola de lote OTLP (`nexus.scheduler.headroom` log + panel Grafana de headroom) libras; aucun renuncia abierta. |
| **B3 - Aprobación de deltas de configuración** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Completo | Vota GOV-2026-03-19 registrar; le paquete signe alimente le pack de telemetrie cite plus bas. |
| **B4 - Repetición de lanzamiento multicarril** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Completado (segundo trimestre de 2026) | Le rerun canary de Q2 a ferme la mitigation du retard TLS; El manifiesto del validador + `.sha256` captura la ubicación de las ranuras 912-936, la semilla de carga de trabajo `NEXUS-REH-2026Q2` y el hash del perfil TLS se registra mientras se vuelve a ejecutar. |

## Calendario trimestral de auditorías de seguimiento enrutado| ID de seguimiento | Fenêtre (UTC) | Resultado | Notas |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Reussi | Admisión a cola P95 est reste bien en dessous de la cible <=750 ms. Requisito de acción aucune. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Reussi | Los hashes de reproducción de OTLP adjuntan un `status.md`; La parite du diff bot SDK confirma la deriva cero. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | resolución | El retardo de TLS se cierra durante la repetición del segundo trimestre; El paquete de telemetría para `NEXUS-REH-2026Q2` registra el hash del perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (ver `artifacts/nexus/tls_profile_rollout_2026q2/`) y cero retardadores. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Reussi | Semilla de carga de trabajo `NEXUS-REH-2026Q2`; paquete de telemetría + manifiesto/resumen en `artifacts/nexus/rehearsals/2026q1/` (rango de ranura 912-936) con agenda en `artifacts/nexus/rehearsals/2026q2/`. |

Los trimestres futuros deben agregar nuevas líneas y reemplazar las entradas finales por un anexo cuando la tabla finaliza el trimestre actual. Consulte esta sección después de las relaciones de seguimiento enrutado o las actas de gobierno utilizando el ancho `#quarterly-routed-trace-audit-schedule`.

## Mitigaciones y elementos de trabajo pendiente| Artículo | Descripción | Propietario | Cíble | Estatuto / Notas |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Termine la propagación del perfil TLS que se retrasará durante `TRACE-CONFIG-DELTA`, capture las preuves du rerun y cierre el registro de mitigación. | @release-eng, @sre-core | Fenetre ruta-traza Q2 2026 | Cierre: captura del hash del perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` en `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; Le rerun para confirmar cero retardataires. |
| Preparación `TRACE-MULTILANE-CANARY` | Programe la repetición Q2, combine los dispositivos con el paquete de telemetría y asegúrese de que los arneses SDK reutilicen el asistente válido. | @telemetry-ops, Programa SDK | Llamada de planificación 2026-04-30 | Completo: agenda almacenada en `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` con ranura para metadatos/carga de trabajo; Reutilización del arnés anotado en el rastreador. |
| Rotación de resumen del paquete de telemetría | Ejecutor `scripts/telemetry/validate_nexus_telemetry_pack.py` antes de cada repetición/liberación y registrar los resúmenes de una base de seguimiento de configuración delta. | @operaciones de telemetría | Candidato de liberación par | Completo - `telemetry_manifest.json` + `.sha256` emisiones en `artifacts/nexus/rehearsals/2026q1/` (rango de ranura `912-936`, semilla `NEXUS-REH-2026Q2`); Resume copias en el tracker y el index des preuves. |

## Integración del paquete de configuración delta- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` reste le resume canonique des diffs. Cuando las novedades `defaults/nexus/*.toml` o los cambios de génesis llegan, mettez a jour ce tracker d'abord puis refletez les pointes cle ici.
- Los paquetes de configuración firmados alimentan el paquete de repetición de telemetría. El paquete, válido según `scripts/telemetry/validate_nexus_telemetry_pack.py`, debe publicarse con las preferencias de configuración delta para que los operadores puedan recargar los artefactos exactos que utilizan el colgante B4.
- Los paquetes Iroha 2 restantes sin carriles: las configuraciones con `nexus.enabled = false` eliminan el mantenimiento de las anulaciones de carril/espacio de datos/enrutamiento solo si el perfil Nexus está activo (`--sora`), no suprima las secciones `nexus.*` des plantillas de un solo carril.
- Gardez le log de vote de gouvernance (GOV-2026-03-19) lie depuis le tracker et esta nota para que les futurs votes puissent copier le format sans devoir re-decouvrir le rituel d'approbation.

## Suivis de repetición de lanzamiento- `docs/source/runbooks/nexus_multilane_rehearsal.md` captura el plan canario, la lista de participantes y las etapas de reversión; Metez a jour le runbook quand la topologie des lanes ou les exporters de telemetrie changent.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` liste cada artefacto verifique durante la repetición del 9 de abril y mantenga el contenido de las notas/agenda de preparación Q2. Agregue las repeticiones futuras al rastreador de memes en lugar de abrir rastreadores ad-hoc para guardar los preuves monótonos.
- Publicar fragmentos del recopilador OTLP y exportaciones Grafana (ver `docs/source/telemetry.md`) cuando los envíos por lotes del exportador cambian; la mise a jour Q1 a porte la taille de lot a 256 echantillons pour evitarr des alertes headroom.
- Les preuves CI/tests multi-lane vivent maintenant dans `integration_tests/tests/nexus/multilane_pipeline.rs` et tournent sous le flowflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), reemplazando la referencia retirada `pytests/nexus/test_multilane_pipeline.py`; Guarde el hash para `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) y sincronícelo con el rastreador durante el procesamiento de paquetes de repetición.

## Ciclo de vida en tiempo de ejecución de carriles- Los planes de ciclo de vida de las líneas en tiempo de ejecución son válidos y mantienen los enlaces de espacio de datos y se cancelan cuando la reconciliación Kura/stockage en paliers echoue, laissant le catalog inchange. Los ayudantes eliminan los relés caché para los carriles retirados porque la síntesis del libro mayor de fusión no se reutiliza de las pruebas obsoletas.
- Aplique los planes a través de los ayudantes de configuración/ciclo de vida Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) para agregar/retirar los carriles sin volver a organizarlos; enrutamiento, instantáneas TEU y registros de manifiestos se recargan automáticamente después de un plan reussi.
- Guía del operador: cuando un plan se hace eco, verifica los espacios de datos disponibles o las raíces de almacenamiento imposibles de creer (raíz fría escalonada/repertorios Kura por carril). Corrigez les chemins de base et reessayez; Los planes se reutilizan para reemitir la diferencia de carril de telemetría/espacio de datos para que los paneles reflejen la nueva topología.

## Telemetría NPoS y prevención de contrapresión

La retro repetición de lanzamiento de la Fase B a demanda de capturas de telemetría determina que el marcapasos NPoS y los sofás de chismes descansan en los límites de la contrapresión. El arnés de integración en `integration_tests/tests/sumeragi_npos_performance.rs` ejercita estos escenarios y ejecuta los currículums JSON (`sumeragi_baseline_summary::<scenario>::...`) cuando llegan nuevas métricas. Lancez-le localement avec:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```Defina `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` o `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` para explorar topologías y tensiones; Los valores por defecto reflejan el perfil de los coleccionistas 1 s/`k=3` utilizado en B4.| Escenario/prueba | Cobertura | Telemetría cle |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloquee 12 rondas con el tiempo de repetición del bloque para registrar los sobres de latencia EMA, los profundidades de archivo y los calibres de envío redundante antes de serializar el paquete de preuves. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Además, el archivo de transacciones garantiza que los aplazamientos de admisión estén activos de manera determinante y que el archivo exporte los contadores de capacidad/saturación. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Echantillonne le jitter du marcapasos y les timeouts de vue justqu'a prouver que la bande +/-125 permille is appliquee. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Pousse de gros payloads RBC jusqu'aux limites soft/hard du store para monitorear las sesiones y los computadores de bytes monten, reculent y se estabilicen sin depasser le store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | La fuerza de las retransmisiones para que los indicadores de relación de envío redundante y los ordenadores de recolectores en el objetivo avancen, lo que demuestra que la telemetría demandada por el retro está ramificada de extremo a extremo. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. || `npos_rbc_chunk_loss_fault_reports_backlog` | Dejar caer los fragmentos a intervalos determinados para verificar que los monitores de acumulación señalan fallas en lugar de drenar silenciando las cargas útiles. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Junte las líneas JSON impresas por el arnés con el raspado Prometheus capturando durante la ejecución cada vez que la gobernanza exige des preuves que las alarmas de contrapresión correspondientes a la topología de repetición.

## Lista de verificación de mise a day

1. Ajoutez de nouvelles fenetres routed-trace et retirez les anciennes lorsque les trimestres tournent.
2. Mettez a jour la table de mitigation apres chaque suivi Alertmanager, meme si la acción consiste en cerrar el ticket.
3. Cuando cambien los deltas de configuración, agregue diariamente el rastreador, esta nota y la lista de resúmenes del paquete de telemetría en la solicitud de extracción de memes.
4. Liez ici tout nouvel artefacto de repetición/telemetría para que los futuros pierdan el día de la hoja de ruta puedan hacer referencia a un solo documento en lugar de notas ad-hoc dispersas.

## Index des preuves| Activo | Emplazamiento | Notas |
|-------|----------|-------|
| Informe de auditoría de seguimiento enrutado (primer trimestre de 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fuente canónica para las preuves Fase B1; espejo para puerta bajo `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Rastreador de configuración delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contiene los currículums de diferencias TRACE-CONFIG-DELTA, las iniciales de los revisores y el registro de votos GOV-2026-03-19. |
| Plan de remediación de telemetría | `docs/source/nexus_telemetry_remediation_plan.md` | Documente el paquete de alerta, la cola de lote OTLP y las barreras de presupuesto de exportación se encuentran en un B2. |
| Rastreador de repetición multicarril | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Liste les artefactos verifica durante la repetición del 9 de abril, validador de manifiesto/resumen, notas/agenda Q2 y preuves de rollback. |
| Manifiesto/resumen del paquete de telemetría (más reciente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Registre la plage 912-936, seed `NEXUS-REH-2026Q2` et les hashes d'artefacts pour les bundles de gouvernance. |
| Manifiesto de perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash du perfil TLS aprueba la captura mientras se vuelve a ejecutar Q2; Citez-le dans les anexos routed-trace. |
| Agenda TRAZA-MULTILANO-CANARIO | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notas de planificación para la repetición del segundo trimestre (valor, rango de espacios, semilla de carga de trabajo, propietarios de acciones). || Runbook de repetición de lanzamiento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Lista de verificación operativa para la puesta en escena -> ejecución -> reversión; mettre a jour quand la topologie des lanes ou les conseils d'exporters changent. |
| Validador de paquetes de telemetría | `scripts/telemetry/validate_nexus_telemetry_pack.py` | Referencia CLI por el retro B4; archive los resúmenes con el rastreador cada vez que el paquete cambia. |
| Regresión multicarril | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Valide `nexus.enabled = true` para las configuraciones de varios carriles, conserve los hashes del catálogo Sora y proporcione los caminos Kura/merge-log por carril (`blocks/lane_{id:03}_{slug}`) a través de `ConfigLaneRouter` antes de publicar los resúmenes de artefactos. |