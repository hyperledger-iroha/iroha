---
lang: es
direction: ltr
source: docs/portal/docs/nexus/operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones-nexus
título: Runbook de operaciones Nexus
descripción: Resumen pronto para uso en campo do flujo de trabajo do operador Nexus, espelhando `docs/source/nexus_operations.md`.
---

Utilice esta página como irmao de referencia rápida de `docs/source/nexus_operations.md`. Ela destila o checklist operacional, os ganchos de gestao de mudanca y os requisitos de cobertura de telemetría que os operadores Nexus deben seguir.

## Lista de ciclo de vida

| Etapa | Acoes | Pruebas |
|-------|--------|----------|
| Pre-voo | Verifique hashes/assinaturas de liberación, confirme `profile = "iroha3"` y prepare plantillas de configuración. | Saida de `scripts/select_release_profile.py`, registro de suma de comprobación, paquete de manifiestos assinado. |
| Actualización del catálogo | Atualize o catalogo `[nexus]`, a politica de roteamento e os limiares de DA conforme o manifesto emitido pelo conselho, y entao capture `--trace-config`. | Saida de `irohad --sora --config ... --trace-config` armazenada com o ticket de onboarding. |
| Humo y corte | Ejecute `irohad --sora --config ... --trace-config`, monte o fume en CLI (`FindNetworkStatus`), valide exportaciones de telemetría y solicite admisión. | Registro de prueba de humo + confirmación de Alertmanager. |
| Estado estavel | Monitoree paneles/alertas, montó rotación de chaves conforme a cadencia de gobierno y sincronizó configuraciones/runbooks cuando los manifiestos mudaron. | Minutas de revisión trimestral, capturas de tableros, ID de tickets de rotacao. |El detalle de incorporación (sustitución de chaves, plantillas de roteamento, pasos del perfil de liberación) permanece en `docs/source/sora_nexus_operator_onboarding.md`.

## Gestao de mudanca

1. **Actualizaciones de lanzamiento** - acompañan anuncios en `status.md`/`roadmap.md`; anexe o checklist de onboarding a cada PR de liberación.
2. **Mudancas de manifesto de lane** - verifique los paquetes asociados a Space Directory y los archivos en `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuración** - toda mudanca em `config/config.toml` solicita un ticket referenciando un carril/espacio de datos. Guarde una copia redigida da configuracao efetiva quando nos entram ou sao atualizados.
4. **Treinos de rollback** - ensaie trimestralmente procedimientos de parada/restauración/humo; registrar resultados en `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprobación de cumplimiento**: las líneas privadas/CBDC deben obtener una evaluación de cumplimiento antes de alterar política de DA o botones de redacción de telemetría (ver `docs/source/cbdc_lane_playbook.md`).

## Telemetría y SLO- Paneles de control: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, más vistas especificaciones de SDK (por ejemplo, `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` e registros de transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métricas a observar:
  - `nexus_lane_height{lane_id}` - alerta para cero progreso por tres slots.
  - `nexus_da_backlog_chunks{lane_id}` - alerta acima dos limiares por carril (padrao 64 público / 8 privado).
  - `nexus_settlement_latency_seconds{lane_id}` - alerta cuando el P99 excede 900 ms (público) o 1200 ms (privado).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerta se a taxa de error de 5 minutos for >2%.
  - `telemetry_redaction_override_total` - Sev 2 inmediatamente; Garantía que anula las multas de cumplimiento de Tenham.
- Execute o checklist de remediacao de telemetria no [Nexus telemetry remediation plan](./nexus-telemetry-remediation) pelo menos trimestralmente y anexo o formulario preenchido nas notas de revisión operacional.

## Matriz de incidentes| Severidad | Definicao | Respuesta |
|----------|------------|----------|
| Septiembre 1 | Violacao de aislamiento de data-space, parada de asentamiento >15 min, ou corrupción de voto de gobernanza. | Acione Nexus Primary + Release Engineering + Compliance, congele admissao, colete artefatos, publique comunicados 30 min, rollout de manifesto falho. | Acione Nexus Primario + SRE, mitigación <=4 h, registro de seguimiento em ate 2 dias uteis. |
| Septiembre 3 | Deriva nao bloqueante (docs, alertas). | Registre no tracker y programe una corrección dentro del sprint. |

Los tickets de incidente incluyen ID de registrador de carril/espacio de datos actualizados, hashes de manifiesto, línea de tiempo, métricas/registros de soporte y tarefas/propietarios de seguimiento.

## Archivo de evidencias

- Paquetes/manifiestos/exportaciones de telemetría de Armazene en `artifacts/nexus/<lane>/<date>/`.
- Mantenha configs redigidas + saya de `--trace-config` para cada liberación.
- Anexe minutas do conselho + decisoes assinadas quando mudancas de config ou manifesto ocorrerem.
- Preservar instantáneas semanales de Prometheus relevantes para métricas Nexus por 12 meses.
- Registre edicoes do runbook em `docs/source/project_tracker/nexus_config_deltas/README.md` para que los auditores saibam quando as responsabilidades mudaram.

## Material relacionado- Visado general: [Nexus descripción general](./nexus-overview)
- Específica: [Especificación Nexus](./nexus-spec)
- Geometria de carriles: [Nexus modelo de carril](./nexus-lane-model)
- Transicao e shims de roteamento: [Nexus notas de transición](./nexus-transition-notes)
- Onboarding de operadores: [Sora Nexus onboarding de operadores](./nexus-operator-onboarding)
- Remediacao de telemetria: [Nexus plan de remediación de telemetría](./nexus-telemetry-remediation)