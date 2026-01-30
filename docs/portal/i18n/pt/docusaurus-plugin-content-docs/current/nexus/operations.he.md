---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/nexus/operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c08666ccc81229bd8e16fd5a9d1795b62a3248effce42d563559c6021512eab3
source_last_modified: "2025-11-14T04:43:20.523068+00:00"
translation_last_reviewed: 2026-01-30
---

Use esta pagina como irmao de referencia rapida de `docs/source/nexus_operations.md`. Ela destila o checklist operacional, os ganchos de gestao de mudanca e os requisitos de cobertura de telemetria que os operadores Nexus devem seguir.

## Lista de ciclo de vida

| Etapa | Acoes | Evidencia |
|-------|--------|----------|
| Pre-voo | Verifique hashes/assinaturas de release, confirme `profile = "iroha3"` e prepare templates de configuracao. | Saida de `scripts/select_release_profile.py`, log de checksum, bundle de manifestos assinado. |
| Alinhamento do catalogo | Atualize o catalogo `[nexus]`, a politica de roteamento e os limiares de DA conforme o manifesto emitido pelo conselho, e entao capture `--trace-config`. | Saida de `irohad --sora --config ... --trace-config` armazenada com o ticket de onboarding. |
| Smoke e cutover | Execute `irohad --sora --config ... --trace-config`, rode o smoke do CLI (`FindNetworkStatus`), valide exportacoes de telemetria e solicite admissao. | Log de smoke-test + confirmacao do Alertmanager. |
| Estado estavel | Monitore dashboards/alertas, rode rotacao de chaves conforme a cadencia de governanca e sincronize configs/runbooks quando manifestos mudarem. | Minutas de revisao trimestral, capturas de dashboards, IDs de tickets de rotacao. |

O onboarding detalhado (substituicao de chaves, templates de roteamento, passos do perfil de release) permanece em `docs/source/sora_nexus_operator_onboarding.md`.

## Gestao de mudanca

1. **Atualizacoes de release** - acompanhe anuncios em `status.md`/`roadmap.md`; anexe o checklist de onboarding a cada PR de release.
2. **Mudancas de manifesto de lane** - verifique bundles assinados do Space Directory e arquive-os em `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuracao** - toda mudanca em `config/config.toml` requer um ticket referenciando a lane/data-space. Guarde uma copia redigida da configuracao efetiva quando nos entram ou sao atualizados.
4. **Treinos de rollback** - ensaie trimestralmente procedimentos de stop/restore/smoke; registre resultados em `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprovacoes de compliance** - lanes privadas/CBDC devem obter aval de compliance antes de alterar politica de DA ou knobs de redacao de telemetria (ver `docs/source/cbdc_lane_playbook.md`).

## Telemetria e SLOs

- Dashboards: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, mais visoes especificas de SDK (por exemplo, `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` e regras de transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Metricas a observar:
  - `nexus_lane_height{lane_id}` - alerta para zero progresso por tres slots.
  - `nexus_da_backlog_chunks{lane_id}` - alerta acima dos limiares por lane (padrao 64 public / 8 private).
  - `nexus_settlement_latency_seconds{lane_id}` - alerta quando o P99 excede 900 ms (public) ou 1200 ms (private).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerta se a taxa de erro de 5 minutos for >2%.
  - `telemetry_redaction_override_total` - Sev 2 imediato; garanta que overrides tenham tickets de compliance.
- Execute o checklist de remediacao de telemetria no [Nexus telemetry remediation plan](./nexus-telemetry-remediation) pelo menos trimestralmente e anexe o formulario preenchido nas notas de revisao operacional.

## Matriz de incidentes

| Severidade | Definicao | Resposta |
|----------|------------|----------|
| Sev 1 | Violacao de isolamento de data-space, parada de settlement >15 min, ou corrupcao de voto de governanca. | Acione Nexus Primary + Release Engineering + Compliance, congele admissao, colete artefatos, publique comunicados <=60 min, RCA <=5 dias uteis. |
| Sev 2 | Violacao de SLA de backlog de lane, ponto cego de telemetria >30 min, rollout de manifesto falho. | Acione Nexus Primary + SRE, mitigue <=4 h, registre follow-ups em ate 2 dias uteis. |
| Sev 3 | Deriva nao bloqueante (docs, alertas). | Registre no tracker e agende a correcao dentro do sprint. |

Tickets de incidente devem registrar IDs de lane/data-space afetadas, hashes de manifesto, timeline, metricas/logs de suporte e tarefas/owners de follow-up.

## Arquivo de evidencias

- Armazene bundles/manifestos/exports de telemetria em `artifacts/nexus/<lane>/<date>/`.
- Mantenha configs redigidas + saida de `--trace-config` para cada release.
- Anexe minutas do conselho + decisoes assinadas quando mudancas de config ou manifesto ocorrerem.
- Preserve snapshots semanais de Prometheus relevantes para metricas Nexus por 12 meses.
- Registre edicoes do runbook em `docs/source/project_tracker/nexus_config_deltas/README.md` para que auditores saibam quando as responsabilidades mudaram.

## Material relacionado

- Visao geral: [Nexus overview](./nexus-overview)
- Especificacao: [Nexus spec](./nexus-spec)
- Geometria de lanes: [Nexus lane model](./nexus-lane-model)
- Transicao e shims de roteamento: [Nexus transition notes](./nexus-transition-notes)
- Onboarding de operadores: [Sora Nexus operator onboarding](./nexus-operator-onboarding)
- Remediacao de telemetria: [Nexus telemetry remediation plan](./nexus-telemetry-remediation)
