---
lang: es
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas-de-transición-nexus
título: Заметки о переходе Nexus
descripción: Зеркало `docs/source/nexus_transition_notes.md`, охватывающее доказательства перехода Phase B, grafic аудита и mitigaции.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Заметки о переходе Nexus

Este registro está listo para usar **Fase B - Nexus Cimientos de transición** para la lista de verificación de la lista de verificación de gran tamaño. En la descripción general de hitos en `roadmap.md` y en la base de datos correspondiente, en la combinación B1-B4, en todos los demás, en la gobernanza, en SRE y en las direcciones. El SDK puede ser eliminado de diferentes sistemas istónicos.

## Область и cadencia

- Monitoreo de seguimiento de rutas y barandillas de telemetría (B1/B2), delta de configuración, gobernanza nueva (B3) y seguimientos del ensayo de lanzamiento de varios carriles (B4).
- Заменяет временную nota de cadencia, которая была здесь раньше; En la auditoría del primer trimestre de 2026, se puede ver una reseña en `docs/source/nexus_routed_trace_audit_report_2026q1.md`, y esta página muestra un gráfico y un registro mitigantes.
- Обновляйте таблицы после каждого окна de seguimiento enrutado, gobierno global y ensayo de lanzamiento. Когда артефакты перемещаются, отражайте новый путь здесь, чтобы downstream docs (estado, paneles, portales SDK) más estables якорь.

## Снимок доказательств (2026 Q1-Q2)| Поток | Доказательства | Propietario(s) | Estado | Примечания |
|------------|----------|----------|--------|-------|
| **B1 - Auditorías de seguimiento enrutado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @operaciones de telemetría, @gobernanza | Завершено (primer trimestre de 2026) | Зафиксированы три окна аудита; TLS лаг `TRACE-CONFIG-DELTA` se cerró en la repetición del segundo trimestre. |
| **B2 - Remediación de telemetría y barandillas** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Завершено | Paquete de alertas, bot de diferencias políticas y lotes OTLP del analizador (`nexus.scheduler.headroom` log + espacio libre del panel Grafana) actualizados; открытых renuncia нет. |
| **B3 - Configurar aprobaciones delta** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Завершено | Голосование GOV-2026-03-19 зафиксировано; подписанный paquete питает paquete de telemetría ниже. |
| **B4 - Ensayo de lanzamiento de varios carriles** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Завершено (segundo trimestre de 2026) | Q2 canary rerun закрыл митигацию TLS лага; manifiesto del validador + `.sha256` archivo de configuración 912-936, semilla de carga de trabajo `NEXUS-REH-2026Q2` y perfil de hash TLS y repetición. |

## Квартальный график auditoría de seguimiento de ruta| ID de seguimiento | Ok (UTC) | Respuesta | Примечания |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Пройдено | Admisión a cola P95 оставался значительно ниже цели <=750 ms. Действий не требуется. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Пройдено | Hashes de reproducción OTLP según `status.md`; paridad SDK diff bot подтвердил нулевой deriva. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Решено | TLS se cierra en la repetición del segundo trimestre; paquete de telemetría para `NEXUS-REH-2026Q2` фиксирует hash perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (см. `artifacts/nexus/tls_profile_rollout_2026q2/`) y ноль отстающих. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Пройдено | Semilla de carga de trabajo `NEXUS-REH-2026Q2`; paquete de telemetría + manifiesto/resumen en `artifacts/nexus/rehearsals/2026q1/` (rango de ranura 912-936) en agenda en `artifacts/nexus/rehearsals/2026q2/`. |

Будущие кварталы должны добавлять новые строки and переносить завершенные записи в приложение, когда таблица перерастет текущий квартал. Utilice esta ruta para rastrear rutas o actas de gobierno de якорь `#quarterly-routed-trace-audit-schedule`.

## Mitigaciones y retrasos| Artículo | Descripción | Propietario | Celo | Estado / Primicia |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Eliminar el perfil TLS, colocarlo en `TRACE-CONFIG-DELTA`, eliminar la repetición de evidencia y eliminar las mitigaciones diarias. | @release-eng, @sre-core | Q2 2026 seguimiento enrutado окно | Закрыто - hash TLS perfil `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` зафиксирован в `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; vuelva a ejecutar подтвердил отсутствие отстающих. |
| Preparación `TRACE-MULTILANE-CANARY` | Запланировать Q2 ensayo, приложить accesorios к paquete de telemetría y убедиться, что SDK arneses переиспользуют валидированный ayudante. | @telemetry-ops, Programa SDK | Planificador 2026-04-30 | Завершено - agenda хранится в `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` с ranura de metadatos/carga de trabajo; reutilizar el arnés отмечен в tracker. |
| Rotación de resumen del paquete de telemetría | Introduzca `scripts/telemetry/validate_nexus_telemetry_pack.py` antes del ensayo/lanzamiento y resúmenes ficticios del conjunto con tracker config delta. | @operaciones de telemetría | На каждый candidato de liberación | Завершено - `telemetry_manifest.json` + `.sha256` выпущены в `artifacts/nexus/rehearsals/2026q1/` (rango de ranura `912-936`, semilla `NEXUS-REH-2026Q2`); digiere скопированы в tracker и индекс доказательств. |

## Paquete delta de configuración de integración- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` остается каноническим resumen de diferencias. Cuando llegue el nuevo `defaults/nexus/*.toml` o su génesis, active el rastreador y presione los puntos siguientes.
- Подписанные paquetes de configuración питают paquete de telemetría de ensayo. Pack, válido `scripts/telemetry/validate_nexus_telemetry_pack.py`, que se adapta a la configuración delta de configuración delta, que los operadores pueden controlar mejor артефакты, использованные в B4.
- Paquetes Iroha 2 instalados en carriles: las configuraciones con `nexus.enabled = false` permiten anular el carril/espacio de datos/enrutamiento, y no se elimina Nexus. профиль (`--sora`), поэтому удаляйте секции `nexus.*` из шаблонов de un solo carril.
- Держите лог голосования Governance (GOV-2026-03-19) связанным с tracker, и с этой заметкой, чтобы будущие голосования могли копировать El formato no es necesario para el ritual de cumplimiento.

## Seguimientos del ensayo de lanzamiento- `docs/source/runbooks/nexus_multilane_rehearsal.md` фиксирует canary план, roster участников и шаги rollback; обновляйте runbook при изменениях топологии lanes o exporter телеметрии.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` перечисляет все артефакты, проверенные на репетиции 9 апреля, и теперь включает Notas/agenda de preparación del Q2. Haga varias repeticiones en el rastreador.
- Publicar fragmentos del recopilador OTLP y exportaciones Grafana (см. `docs/source/telemetry.md`) para exportar guías de procesamiento por lotes; El primer trimestre del año incluye un tamaño de lote de 256 muestras, lo que genera alertas de margen de maniobra.
- La etapa CI/pruebas de múltiples carriles se conecta a `integration_tests/tests/nexus/multilane_pipeline.rs` y se configura en el flujo de trabajo `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), заменяя. устаревшую ссылку `pytests/nexus/test_multilane_pipeline.py`; держите hash `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) sincronizado con el rastreador de paquetes de ensayo nuevos.

## Ciclo de vida del carril de ejecución- El ciclo de vida del carril de tiempo de ejecución permite validar los enlaces del espacio de datos y realizar la reconciliación de Kura/almacenamiento por niveles, y el catálogo está disponible según la configuración. Los ayudantes incluyen relés de carril de carriles retirados, síntesis de libro mayor de fusión y pruebas de uso permanente.
- Primero, planifique los asistentes de configuración/ciclo de vida de Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) para los carriles de inicio/cambio antes de reiniciar; enrutamiento, instantáneas de TEU y registros de manifiestos según el plan de uso.
- Operador principal: para proteger los espacios de datos externos y las raíces de almacenamiento, no es necesario utilizar directorios raíz fríos escalonados/carril Kura. Исправьте базовые пути и повторите; успешные планы повторно эмитят telemetry diff lane/dataspace, чтобы paneles de control отражали новую топологию.

## NPoS телеметрия и доказательства contrapresión

El ensayo de lanzamiento de la Fase B de Retro запросило детерминированные телеметрийные capturas telemétricas, доказывающие, что marcapasos NPoS y chismes слои остаются в пределах contrapresión. El arnés integrado en `integration_tests/tests/sumeragi_npos_performance.rs` muestra estos escenarios y reproduce resúmenes JSON (`sumeragi_baseline_summary::<scenario>::...`) según nuevas métricas. Запуск локально:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Instale `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` o `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R`, para crear topologías muy estresantes; значения по умолчанию отражают профиль 1 s/`k=3`, isпользованный в B4.| Сценарий / prueba | Покрытие | Telemetría clásica |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Incluye 12 rondas de tiempo de bloque de ensayo, elimina sobres de latencia de EMA, glubinas adicionales y medidores de envío redundante antes del paquete de pruebas en serie. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Переполняет очередь транзакций, чтобы garantировать детерминированный запуск aplazamientos de admisión y экспорт счетчиков capacidad/saturación. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Para comprobar la fluctuación del marcapasos y ver los tiempos de espera, no se deben aplicar polos de +/-125 por mil. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Las cargas útiles RBC de la cadena de bloques se almacenan con límites blandos/difíciles, se guardan listas, se almacenan y se estabilizan sesiones/bytes en el almacén permanente. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Para retransmitir, чтобы medidores de relación de envío redundante y contadores de colectores en el objetivo, доказывая de extremo a extremo связность требуемой телеметрии. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Se distribuyen fragmentos determinados, que monitorean el trabajo pendiente y pueden fallar en todas las cargas útiles de drenaje. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |Utilice líneas JSON, utilice arnés, conecte con Prometheus scrape, elimine el proceso de gobernanza del sistema доказательства, что alarmas de contrapresión соответствуют топологии de ensayo.

## Чеклист обновления

1. Agregue nuevas señales de seguimiento de ruta y perenó las estrellas en algunos apartamentos.
2. Desactive la tabla de mitigación del seguimiento del registro Alertmanager, para eliminar el ticket.
3. Actualice los deltas de configuración, actualice el rastreador, esta configuración y resuelva el paquete de telemetría en una solicitud de extracción.
4. Ссылайтесь здесь на новые rehearsal/telemetry артефакты, чтобы будущие обновления roadmap ссылались на единый документ, а не разрозненные ad-hoc заметки.

## Индекс доказательств| Activo | Ubicación | Примечания |
|-------|----------|-------|
| Informe de auditoría de seguimiento enrutado (primer trimestre de 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Канонический источник доказательств Fase B1; зеркалируется в портал через `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Configurar rastreador delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Содержит TRACE-CONFIG-DELTA resúmenes de diferencias, revisores iniciales y registros de registro GOV-2026-03-19. |
| Plan de remediación de telemetría | `docs/source/nexus_telemetry_remediation_plan.md` | Paquete de alertas de documentación, tamaño de lote de OTLP y barreras de presupuesto de exportación, incluido en B2. |
| Rastreador de ensayos de varios carriles | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Писок артефактов репетиции 9 апреля, manifiesto/resumen del validador, notas/agenda del segundo trimestre y evidencia de reversión. |
| Manifiesto/resumen del paquete de telemetría (más reciente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Rango de ranuras de chips 912-936, semillas `NEXUS-REH-2026Q2` y paquetes de gobernanza de artefactos hashes. |
| Manifiesto de perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash утвержденного TLS профиля, захваченный в Q2 rerun; Consulte los apéndices de seguimiento de ruta. |
| Agenda TRACE-MULTILANO-CANARIA | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Плановые заметки для Q2 ensayo (ventana, rango de espacios, semilla de carga de trabajo, propietarios de acciones). |
| Lanzamiento del runbook de ensayo | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Операционный чеклист puesta en escena -> ejecución -> reversión; обновлять при изменении топологии carriles или exportadores de orientación. || Validador de paquetes de telemetría | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI, insertado en B4 retro; архивируйте resume вместе с tracker при любом изменении pack. |
| Regresión multicarril | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Proporcionar `nexus.enabled = true` para configuraciones de varios carriles, agregar hashes de catálogo de Sora y proporcionar rutas de registro de combinación/Kura locales de carril (`blocks/lane_{id:03}_{slug}`) antes de `ConfigLaneRouter` resúmenes de artefactos publicados. |