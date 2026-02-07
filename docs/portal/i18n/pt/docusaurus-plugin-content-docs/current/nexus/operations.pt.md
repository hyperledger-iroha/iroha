---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nexo
título: Runbook de operações Nexus
description: Resumo pronto para uso no campo do fluxo de trabalho do operador Nexus, espelhando `docs/source/nexus_operations.md`.
---

Use esta página como irmão de referência rápida de `docs/source/nexus_operations.md`. Ela destila o checklist operacional, os ganchos de gestão de mudança e os requisitos de cobertura de telemetria que os operadores Nexus devem seguir.

## Lista de ciclo de vida

| Etapa | Aços | Evidência |
|-------|--------|----------|
| Pré-voo | Verifique hashes/assinaturas de lançamento, confirme `profile = "iroha3"` e prepare templates de configuração. | Saida de `scripts/select_release_profile.py`, log de checksum, pacote de manifestos assinados. |
| Alinhamento do catálogo | Atualizar o catálogo `[nexus]`, a política de roteamento e os limites de DA conforme o manifesto emitido pelo conselho, e então capturar `--trace-config`. | Saida de `irohad --sora --config ... --trace-config` armazenada com o ticket de onboarding. |
| Fumaça e corte | Execute `irohad --sora --config ... --trace-config`, rode o smoke do CLI (`FindNetworkStatus`), valide exportações de telemetria e solicite admissão. | Log de smoke-test + confirmação do Alertmanager. |
| Estado estavel | Monitore painéis/alertas, rode rotação de chaves conforme a cadência de governança e sincronize configurações/runbooks quando os manifestos mudam. | Minutas de revisão trimestral, capturas de dashboards, IDs de tickets de rotação. |

O onboarding detalhado (substituição de chaves, templates de roteamento, passos do perfil de release) permanece em `docs/source/sora_nexus_operator_onboarding.md`.

## Gestão de mudança

1. **Atualizações de lançamento** - acompanhe anúncios em `status.md`/`roadmap.md`; anexo ou lista de verificação de integração a cada PR de lançamento.
2. **Mudancas de manifesto de lane** - verifique pacotes selecionados do Space Directory e arquive-os em `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuração** - toda mudança em `config/config.toml` requer um ticket referenciando uma pista/espaço de dados. Guarde uma cópia redigida da configuração efetiva quando nós entrarmos ou forem atualizados.
4. **Treinos de rollback** - ensaie trimestralmente procedimentos de parar/restaurar/smoke; registre resultados em `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprovações de conformidade** - faixas privadas/CBDC devem obter avaliação de conformidade antes de alterar a política de DA ou botões de redacção de telemetria (ver `docs/source/cbdc_lane_playbook.md`).

## Telemetria e SLOs

- Dashboards: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, mais especificações do SDK (por exemplo, `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` e regras de transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métricas a observar:
  - `nexus_lane_height{lane_id}` - alerta para zero progresso por três slots.
  - `nexus_da_backlog_chunks{lane_id}` - alerta acima dos limites por lane (padrão 64 público / 8 privado).
  - `nexus_settlement_latency_seconds{lane_id}` - alerta quando o P99 ultrapassa 900 ms (público) ou 1200 ms (privado).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerta se a taxa de erro de 5 minutos for >2%.
  - `telemetry_redaction_override_total` - 2 de setembro imediatamente; garanta que as substituições tenham tickets de conformidade.
- Executar o checklist de remediação de telemetria no [Nexus telemetry remediation plan](./nexus-telemetry-remediation) pelo menos trimestralmente e anexo ou formulário preenchido nas notas de revisão operacional.

## Matriz de incidentes

| Severidade | Definição | Resposta |
|----------|------------|----------|
| 1º de setembro | Violação de isolamento de espaço de dados, parada de liquidação >15 min, ou corrupção de voto de governança. | Acione Nexus Primary + Release Engineering + Compliance, congele admissao, colete artefatos, comunicados públicos <=60 min, RCA <=5 dias uteis. |
| 2 de setembro | Violação de SLA de backlog de pista, ponto cego de telemetria >30 min, rollout de manifesto falho. | Acione Nexus Primário + SRE, mitigar <=4 h, registrar acompanhamentos em até 2 dias uteis. |
| 3 de setembro | Deriva não bloqueador (docs, alertas). | Cadastre-se no tracker e agende a correção dentro do sprint. |

Tickets de incidente incidente IDs de registrador de pista/espaço de dados afetados, hashes de manifesto, cronograma, métricas/logs de suporte e tarefas/proprietários de acompanhamento.

## Arquivo de evidências

- Armazene pacotes/manifestos/exportações de telemetria em `artifacts/nexus/<lane>/<date>/`.
- Mantenha configurações redigidas + saida de `--trace-config` para cada lançamento.
- Anexo minutas do conselho + decisões assinadas quando mudanças de configuração ou manifesto ocorrerem.
- Preservar snapshots semanais de Prometheus relevantes para métricas Nexus por 12 meses.
- Cadastre as edições do runbook em `docs/source/project_tracker/nexus_config_deltas/README.md` para que os auditores saibam quando as responsabilidades mudaram.## Material relacionado

- Visão geral: [Nexus Overview](./nexus-overview)
- Especificação: [especificação Nexus](./nexus-spec)
- Geometria de pistas: [modelo de pista Nexus] (./nexus-lane-model)
- Transição e calços de roteamento: [Notas de transição Nexus](./nexus-transition-notes)
- Onboarding de operadores: [Sora Nexus operador onboarding](./nexus-operator-onboarding)
- Remediação de telemetria: [Plano de remediação de telemetria Nexus](./nexus-telemetry-remediation)