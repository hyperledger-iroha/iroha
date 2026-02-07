---
lang: es
direction: ltr
source: docs/portal/docs/nexus/transition-notes.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas-de-transición-nexus
título: Notas de transición del Nexus
descripción: Espelho de `docs/source/nexus_transition_notes.md`, cobrindo evidencia de transicao da Phase B, o calendario de auditoria e as mitigacoes.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notas de transición de Nexus

Este registro acompaña el trabajo pendiente de **Phase B - Nexus Transition Foundations** ate que a checklist de lancamento multi-lane termine. Ele complementa as entradas de hitos en `roadmap.md` y mantiene una evidencia referenciada por B1-B4 en un único lugar para que gobiernen, SRE y líderes de SDK compartilhem a mesma fonte de verdade.

## Escopo y cadencia

- Cobre como auditorios enrutados y barandillas de telemetría (B1/B2), o conjunto de deltas de configuración aprobado por gobierno (B3) y os acompañamientos de ensaio de lancamento multicarril (B4).
- Sustitui a nota temporaria de cadencia que antes vivía aquí; A partir de la auditoría del primer trimestre de 2026, el informe detallado se encuentra en `docs/source/nexus_routed_trace_audit_report_2026q1.md`, mientras que esta página mantiene el calendario corrente y el registro de mitigadores.
- Atualizar como tabelas apos cada janela routed-trace, voto degobernanza ou ensaio de lancamento. Cuando los artefactos se mueven, reflita una nueva localización dentro de esta página para que los documentos posteriores (estado, paneles, portais SDK) puedan vincular un ancoradouro estavel.

## Instantánea de evidencia (2026 Q1-Q2)| Flujo de trabajo | Pruebas | Propietario(s) | Estado | Notas |
|------------|----------|----------|--------|-------|
| **B1 - Auditorías de seguimiento enrutado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @operaciones de telemetría, @gobernanza | Completo (Q1 2026) | Tres janelas de auditoria registradas; o atraso TLS de `TRACE-CONFIG-DELTA` foi fechado durante la repetición de Q2. |
| **B2 - Reparación de telemetría y barandillas** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Completo | Paquete de alerta, política de diferenciación de bot y tamaño de lote OTLP (`nexus.scheduler.headroom` log + panel Grafana de headroom) enviados; sem renuncias em aberto. |
| **B3 - Aprovacoes de deltas de configuracao** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Completo | Voto GOV-2026-03-19 registrado; o paquete assinado alimenta o paquete de telemetria citado abaixo. |
| **B4 - Ensaio de lancamento multicarril** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Completo (Q2 2026) | O vuelva a ejecutar canary de Q2 fechou a mitigacao do atraso TLS; o manifiesto del validador + captura `.sha256` o intervalo de ranuras 912-936, semilla de carga de trabajo `NEXUS-REH-2026Q2` y hash del perfil TLS registrado sin repetición. |

## Calendario trimestral de auditorias routed-trace| ID de seguimiento | Janela (UTC) | Resultado | Notas |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Aprovado | Entrada cola P95 ficou bem abaixo do alvo <=750 ms. Nenhuma acao necesaria. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Aprovado | Hashes de reproducción OTLP anexados a `status.md`; La paridad del bot de diferenciación del SDK confirma la deriva cero. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Resuelto | O atraso TLS foi fechado durante la repetición de Q2; El paquete de telemetría para `NEXUS-REH-2026Q2` registra el hash del perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (ver `artifacts/nexus/tls_profile_rollout_2026q2/`) y cero atrasados. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Aprovado | Semilla de carga de trabajo `NEXUS-REH-2026Q2`; paquete de telemetría + manifiesto/digest en `artifacts/nexus/rehearsals/2026q1/` (rango de ranura 912-936) con agenda en `artifacts/nexus/rehearsals/2026q2/`. |

Los trimestres futuros deben agregar nuevas líneas y moverse como entradas concluidas para un apéndice cuando la tabla crece además del trimestre actual. Referencia esta secao a partir de relatorios routed-trace o atas de gobierno usando un ancora `#quarterly-routed-trace-audit-schedule`.

## Mitigacoes e items de backlog| Artículo | Descripción | Propietario | alvo | Estado / Notas |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Finalizar la propagación del perfil TLS que ficou atrasado durante `TRACE-CONFIG-DELTA`, capturar evidencia de la repetición y cerrar el registro de mitigacao. | @release-eng, @sre-core | Janela enrutada-traza del segundo trimestre de 2026 | Fechado - hash del perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capturado en `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; o volver a ejecutar confirmou que nao ha atrasados. |
| Preparación `TRACE-MULTILANE-CANARY` | Programar el ensayo de Q2, agregar accesorios al paquete de telemetría y garantizar que los arneses SDK se reutilicen o el asistente validado. | @telemetry-ops, Programa SDK | Chamada de planejamento 2026-04-30 | Completo - agenda armazenada em `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` com metadados de slot/workload; reutilizacao do arnés anotado sin rastreador. |
| Rotación de resumen del paquete de telemetría | Ejecute `scripts/telemetry/validate_nexus_telemetry_pack.py` antes de cada ensayo/liberación y los resúmenes del registrador al lado del rastreador de configuración delta. | @operaciones de telemetría | Por liberación candidato | Completo - `telemetry_manifest.json` + `.sha256` emitidos en `artifacts/nexus/rehearsals/2026q1/` (rango de ranuras `912-936`, semilla `NEXUS-REH-2026Q2`); resúmenes copiados no tracker e no indice de evidencia. |

## Integración del paquete de configuración delta- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` sigue como resumen canónico de diferencias. Quando chegarem novos `defaults/nexus/*.toml` ou mudanzas de genesis, atualize esse tracker primeiro e depois reflita os destaques aqui.
- Los paquetes de configuración firmados alimentan el paquete de telemetría de ensayo. El paquete, validado por `scripts/telemetry/validate_nexus_telemetry_pack.py`, debe ser publicado junto con una evidencia de configuración delta para que los operadores puedan reproducir los artefatos exatos usados ​​durante B4.
- Los paquetes de Iroha 2 permanecen en los carriles: configs com `nexus.enabled = false` ahora rejeitam overrides de lane/dataspace/routing a menos que el perfil Nexus esteja habilitado (`--sora`), entonces se elimina como secoes `nexus.*` das plantillas de un solo carril.
- Mantenha o log de voto degobernanza (GOV-2026-03-19) linkado tanto no tracker quanto nesta nota para que futuros votos puedan copiar o formato sin redescobrir o ritual de aprovacao.

## Acompañamientos del ensayo de lancamento- `docs/source/runbooks/nexus_multilane_rehearsal.md` captura del plano canario, lista de participantes y pasos de rollback; Actualizar el runbook cuando la topología de carriles o los exportadores de telemetría mudarem.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lista cada artefato checado durante el ensayo de 9 de abril e agora inclui notas/agenda de preparacao Q2. Adicione ensaios futuros ao mesmo tracker em vez de abrir trackers isolados para mantener la evidencia monótona.
- Fragmentos públicos del archivo OTLP y exportaciones del Grafana (ver `docs/source/telemetry.md`) cuando la orientación del procesamiento por lotes del exportador cambia; La actualización del Q1 aumentó el tamaño del lote a 256 muestras para evitar alertas de espacio libre.
- A evidencia de CI/tests multi-lane agora vive em `integration_tests/tests/nexus/multilane_pipeline.rs` e roda sob o flowflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), sustituyendo una referencia aposentada `pytests/nexus/test_multilane_pipeline.py`; Mantenga el hash de `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) en sincronización con el rastreador para actualizar paquetes de ensayo.

## Ciclo de vida de carriles en tiempo de ejecución- Los planos de ciclo de vida de carriles en tiempo de ejecución ahora validan enlaces de espacio de datos y abortan cuando se reconciliacao Kura/armazenamento em camadas falha, manteniendo o catálogo inalterado. Los ayudantes pueden retransmitir carriles en caché para carriles aposentados, para que un libro mayor de fusión sintético no reutilice pruebas obsoletas.
- Aplique planos pelos helpers de config/lifecycle do Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) para agregar/retirar carriles sin reiniciar; enrutamiento, instantáneas TEU y registros de manifiestos se recarregam automáticamente apos um plano bem-sucedido.
- Orientación para operadores: cuando un plano falha, verifique los espacios de datos ausentes o las raíces de almacenamiento que no pueden ser criados (tiered cold root/diretorios Kura por lane). Corrija os caminhos base e tente novamente; Los planos exitosos reemiten la diferencia de telemetría de carril/espacio de datos para que los paneles reflejen una nueva topología.

## Telemetría NPoS y evidencia de contrapresión

O retro do ensaio de lancamento da Phase B pediu capturas de telemetría deterministas que demuestran que el marcapasos NPoS y como camadas de chismes permanecen dentro de sus límites de contrapresión. El arnés de integración en `integration_tests/tests/sumeragi_npos_performance.rs` ejercita estos escenarios y emite resúmenes JSON (`sumeragi_baseline_summary::<scenario>::...`) cuando nuevas métricas chegam. Ejecutar localmente com:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```Defina `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` o `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` para explorar topologías de mayor estrés; os valores padrao reflejam o perfil de coletores 1 s/`k=3` usado em B4.| Cenario / prueba | Cobertura | Telemetria chave |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloqueia 12 rodadas con o block time do ensaio para registrar sobres de latencia EMA, profundidades de fila y calibres de envío redundante antes de serializar o paquete de evidencia. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inunda a fila de transacoes para garantizar que as aplazamientos de admisión sejam activadas de forma determinista e que a fila exporte contadores de capacidade/saturacao. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Mostramos la fluctuación del marcapasos y los tiempos de espera de visualización que provar que una banda +/-125 por mil y aplicada. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Empurra payloads RBC grandes ate os limites soft/hard do store para mostrar que sesiones y contadores de bytes sobem, recuam y se estabilizam sem ultrapassar o store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Forca retransmisiones para que los medidores de relación de envío redundante y los contadores de recolectores en el objetivo avancen, provocando que una telemetría pedida pelo retro esté conectada de extremo a extremo. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. || `npos_rbc_chunk_loss_fault_reports_backlog` | Descarte trozos en intervalos deterministas para verificar que los monitores de backlog levantan falhas en vez de drenar silenciosamente las cargas útiles. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Anexe como líneas JSON que el arnés imprime junto con el raspado del Prometheus capturado durante la ejecución siempre que el gobierno solicita evidencia de que las alarmas de contrapresión corresponden a la topología del ensayo.

## Lista de verificación de actualización

1. Adicione novas janelas routed-trace e retire as antigas quando os trimestres girarem.
2. Realice una tabla de mitigacao apos cada acompañamiento de Alertmanager, mesmo que acao seja fechar o ticket.
3. Cuando los cambios de configuración cambian, actualizan el rastreador, estas notas y una lista de resúmenes del paquete de telemetría no incluyen ninguna solicitud de extracción.
4. Linke aqui qualquer novo artefato de ensaio/telemetria para que futuras actualizaciones de roadmap possam referenciar um unico documento em vez de notas ad-hoc dispersas.

## Índice de evidencia| activo | Localizacao | Notas |
|-------|----------|-------|
| Relatorio de auditoria routed-trace (T1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fuente canónica da evidencia de la Fase B1; escrito para el portal en `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Rastreador de configuración delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contem os resumos de diffs TRACE-CONFIG-DELTA, iniciais de revisores y o log de voto GOV-2026-03-19. |
| Plano de reparación de telemetría | `docs/source/nexus_telemetry_remediation_plan.md` | Documenta o alert pack, o tamanho de lote OTLP y os guardrails de orcamento de exportacao vinculados a B2. |
| Tracker de ensayo multicarril | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Lista de artefactos del ensayo del 9 de abril, manifiesto/digest del validador, notas/agenda Q2 y evidencia de rollback. |
| Manifiesto/resumen del paquete de telemetría (más reciente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Registre el rango de ranura 912-936, la semilla `NEXUS-REH-2026Q2` y los hashes de artefatos para paquetes de gobierno. |
| Manifiesto de perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash del perfil TLS aprobado capturado durante la repetición del Q2; citar en los apéndices routed-trace. |
| Agenda TRAZA-MULTILANO-CANARIO | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notas de planejamento para el ensayo Q2 (janela, rango de tragamonedas, semilla de carga de trabajo, propietarios de acoes). |
| Runbook de ensayo de lancamento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Lista de verificación operativa para preparación -> ejecución -> reversión; actualizar cuando se muda la topología de carriles o la orientación de los exportadores. || Validador de paquetes de telemetría | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI referenciado pelo retro B4; archivar resúmenes al lado del rastreador siempre que el paquete mudar. |
| Regresión multicarril | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Prova `nexus.enabled = true` para configuraciones de varios carriles, conserva los hashes del catálogo Sora y provisiona caminos Kura/merge-log por carril (`blocks/lane_{id:03}_{slug}`) vía `ConfigLaneRouter` antes de publicar resúmenes de artefatos. |