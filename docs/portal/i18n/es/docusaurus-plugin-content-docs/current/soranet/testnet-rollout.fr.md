---
lang: es
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lanzamiento de testnet
título: Implementación del testnet SoraNet (SNNet-10)
sidebar_label: Implementación de testnet (SNNet-10)
Descripción: Plan de activación por fases, kit de incorporación y puertas de telemetría para las promociones de testnet SoraNet.
---

:::nota Fuente canónica
Esta página refleja el plan de implementación de SNNet-10 en `docs/source/soranet/testnet_rollout_plan.md`. Gardez les deux copys alignees jusqu'a la retraite des docs historiques.
:::

SNNet-10 coordina la activación mediante etapas de superposición anónima de SoraNet en todas las fuentes. Utilice este plan para traducir la hoja de ruta en espacios concretos, runbooks y puertas de telemetría para que cada operador comprenda los asistentes antes de que SoraNet realice el transporte por defecto.

## Fases de lanzamiento| Fase | Calendario (cible) | Portée | Requisitos de artefactos |
|-------|-------------------|-------|--------------------|
| **T0 - Testnet cerrado** | Cuarto trimestre de 2026 | 20-50 retransmisiones en >=3 ASN operan por los contribuyentes principales. | Kit de testnet de incorporación, conjunto de fijación de guardia de humo, línea base de latencia + métricas PoW, registro de simulacro de apagón. |
| **T1 - Beta pública** | Primer trimestre de 2027 | >=100 relés, rotación de guardia activa, vinculación de salida impuesta, SDK betas por defecto en SoraNet con `anon-guard-pq`. | Kit de incorporación diaria, lista de verificación del operador de verificación, SOP de publicación del directorio, paquete de paneles de telemetría, informes de ensayo del incidente. |
| **T2 - Red principal por defecto** | Q2 2027 (condición a la finalización SNNet-6/7/9) | El equipo de producción pasó por defecto a SoraNet; transporta obfs/MASQUE et programming du PQ ratchet actives. | Actas de aprobación de gobierno, procedimiento de reversión directa únicamente, alarmas de degradación, informe de firma de medidas de éxito. |

Il n'y a **aucune voie de saut** - cada fase doit livrer la telemetrie et les artefactos de gobernanza de la etapa precedente antes de la promoción.

## Kit de testnet de incorporación

Cada operador de retransmisión recupera un paquete determinado con los archivos siguientes:| Artefacto | Descripción |
|----------|-------------|
| `01-readme.md` | Vista de conjunto, puntos de contacto y calendario. |
| `02-checklist.md` | Lista de verificación previa al vuelo (hardware, investigación de accesibilidad, política de verificación de la guardia). |
| `03-config-example.toml` | Configuración mínima de relé + orquestador SoraNet alineado en los bloques de cumplimiento SNNet-9, incluido un bloque `guard_directory` que fija el hash del último protector de instantáneas. |
| `04-telemetry.md` | Instrucciones para conectar los paneles de métricas de privacidad SoraNet y las siguientes señales de alerta. |
| `05-incident-playbook.md` | Procedimiento de respuesta de caída/bajada de categoría con matriz de escalada. |
| `06-verification-report.md` | Modelo que los operadores remplissent y retournent una vez que las pruebas de humo sean válidas. |

Una copia impresa está disponible en `docs/examples/soranet_testnet_operator_kit/`. Chaque promoción rafraichit le kit; los números de versión siguiente a la fase (por ejemplo, `testnet-kit-vT0.1`).

Para los operadores public-beta (T1), los breves resumenes en `docs/source/soranet/snnet10_beta_onboarding.md` resumen los requisitos previos, los espacios de telemetría y el flujo de trabajo de entrega en comparación con el kit determinante y los asistentes de validación.`cargo xtask soranet-testnet-feed` genera el feed JSON que agrega la ventana de promoción, la lista de relevos, la relación de métricas, las preuves de taladros y los hashes de piezas unidas referenciadas por la plantilla stage-gate. Signez d'abord les logs de taladrados y les piezas unidas con `cargo xtask soranet-testnet-drill-bundle` hasta que le feed registre `drill_log.signed = true`.

## Métricas de éxito

La promoción entre fases está condicionada por la telemetría siguiente, recogida durante las siguientes dos semanas:

- `soranet_privacy_circuit_events_total`: 95% de los circuitos determinados sin caída de tensión ni degradación; El 5% restante está limitado por la oferta de PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: =99% dans la fenetre cible de 3 s; informe a través de `soranet_privacy_throttles_total{scope="congestion"}`.
- Latencia (percentil 95) por región: <200 ms una vez que los circuitos están completos, capturados a través de `soranet_privacy_rtt_millis{percentile="p95"}`.

Las plantillas de paneles y alertas viven en `dashboard_templates/` y `alert_templates/`; Recopie los datos en su repositorio de telemetría y agregue las comprobaciones de lint CI. Utilice `cargo xtask soranet-testnet-metrics` para generar la relación de gobierno orientado hacia la demanda de promoción.Les soumissions stage-gate doivent suivre `docs/source/soranet/snnet10_stage_gate_template.md`, qui renvoie vers le form Markdown pret a copier sous `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Lista de verificación de verificación

Los operadores deben validar los puntos siguientes antes de entrar en cada fase:

- ✅ Retransmitir la firma del anuncio con el sobre de admisión corriente.
- ✅ Prueba de humo de rotación de guardia (`tools/soranet-relay --check-rotation`) válida.
- ✅ `guard_directory` pointe vers le dernier artefacto `GuardDirectorySnapshotV2` et `expected_directory_hash_hex` corresponden al resumen del comité (le demarrage du Relay periodise le hash valide).
- ✅ Les metrics de PQ ratchet (`sorafs_orchestrator_pq_ratio`) descansan au-dessus des seuils cibles pour l'etape demandee.
- ✅ La configuración de cumplimiento GAR corresponde a la última etiqueta (vea el catálogo SNNet-9).
- ✅ Simulación de bajada de alarma (desactive los coleccionistas, espere una alerta durante 5 minutos).
- ✅ Ejecutar Drill PoW/DoS con las etapas de mitigación documentadas.

Un modelo previo al reemplazo está incluido en el kit de incorporación. Los operadores mantienen una relación completa con el gobierno del servicio de asistencia técnica antes de recibir las credenciales de producción.

## Gobernanza y presentación de informes- **Control de cambios:** las promociones exigen una aprobación del Consejo de Gobierno registrada en las actas del consejo y unidas a la página de estado.
- **Resumen de estado:** publique des mises a jour hebdomadaires resument le nombre de Relays, le ratio PQ, les incidentes de apagón y les elementos de acción en atención (stocke dans `docs/source/status/soranet_testnet_digest.md` une fois la cadence lancee).
- **Rollbacks:** mantenga un plan de rollback firmado que ramene el rescate en la fase anterior en 30 minutos, incluida la invalidación de DNS/guard cache y las plantillas de comunicación del cliente.

## Actividades de soporte

- `cargo xtask soranet-testnet-kit [--out <dir>]` materializa el kit de incorporación desde `xtask/templates/soranet_testnet/` al repertorio disponible (por defecto `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` evaluó las métricas de éxito SNNet-10 y emet una estructura de relación pasa/falla adaptando la gobernanza de las revistas. Una instantánea del ejemplo se encuentra en `docs/examples/soranet_testnet_metrics_sample.json`.
- Las plantillas Grafana y Alertmanager viven en `dashboard_templates/soranet_testnet_overview.json` y `alert_templates/soranet_testnet_rules.yml`; Copie los archivos en su repositorio de telemetría o ramifique los controles de pelusa CI.
- La plantilla de degradación de comunicación para el SDK/portal de mensajes reside en `docs/source/soranet/templates/downgrade_communication_template.md`.
- Los resúmenes de estatus hebdomadaires deben utilizar `docs/source/status/soranet_testnet_weekly_digest.md` como forma canónica.

Las solicitudes de extracción deben cambiar cada día esta página con todos los cambios de artefactos o telemetría según el plan de implementación restante canónico.