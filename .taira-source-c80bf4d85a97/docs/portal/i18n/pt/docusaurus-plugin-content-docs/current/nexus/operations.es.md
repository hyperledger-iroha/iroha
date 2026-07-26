---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nexo
título: Runbook de operações de Nexus
description: Lista de resumo para o campo do fluxo de trabalho do operador de Nexus, que reflete `docs/source/nexus_operations.md`.
---

Use esta página como o irmão de referência rápida de `docs/source/nexus_operations.md`. Retome a lista operacional, os ganchos de gerenciamento de mudanças e os requisitos de cobertura de telemetria que os operadores de Nexus devem seguir.

## Lista de ciclo de vida

| Etapa | Ações | Evidência |
|-------|--------|----------|
| Pré-voo | Verifique hashes/firmas de lançamento, confirme `profile = "iroha3"` e prepare plantas de configuração. | Saída de `scripts/select_release_profile.py`, registro de checksum, pacote de manifestos firmado. |
| Alinhamento do catálogo | Atualiza o catálogo `[nexus]`, a política de enrutamiento e os umbrais de DA após o manifesto emitido pelo conselho, e depois a captura `--trace-config`. | Saída de `irohad --sora --config ... --trace-config` armazenada com o ticket de embarque. |
| Testes de humor e corte | Execute `irohad --sora --config ... --trace-config`, execute o teste de humor do CLI (`FindNetworkStatus`), valide as exportações de telemetria e solicite admissão. | Log de teste de fumaça + confirmação do Alertmanager. |
| Estado estável | Monitore painéis/alertas, gire as chaves de acordo com a cadência de governança e sincronize configurações/runbooks quando mudar os manifestos. | Minutas de revisão trimestral, capturas de dashboards, IDs de tickets de rotação. |

O detalhe de integração (substituição de chaves, plantas de enrutamiento, passos do perfil de lançamento) permanece em `docs/source/sora_nexus_operator_onboarding.md`.

## Gestão de mudanças

1. **Atualizações de lançamento** - siga anúncios em `status.md`/`roadmap.md`; complementa a lista de verificação de integração a cada PR de lançamento.
2. **Alterações de manifestação de pista** - verifica pacotes firmados no Diretório Espacial e arquivados abaixo de `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuração** - cada mudança em `config/config.toml` requer um ticket que referencie a pista/espaço de dados. Guarde uma cópia editada da configuração efetiva quando os nós forem atualizados ou atualizados.
4. **Simulacros de rollback** - ensai trimestralmente procedimentos de parar/restaurar/smoke; registre resultados em `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprovações de conformidade** - vias privadas/CBDC devem garantir o bom visto de conformidade antes de modificar a política de DA ou os botões de redação de telemetria (ver `docs/source/cbdc_lane_playbook.md`).

## Telemetria e SLOs

- Painéis: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, mas com vistas específicas do SDK (por exemplo, `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` e regras de transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métricas a vigilante:
  - `nexus_lane_height{lane_id}` - alerta se não houver progresso durante três slots.
  - `nexus_da_backlog_chunks{lane_id}` - alerta por encima de umbrales por lane (por defeito 64 público / 8 privado).
  - `nexus_settlement_latency_seconds{lane_id}` - alerta quando o P99 supera 900 ms (público) ou 1200 ms (privado).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerta se a taxa de erro for 5 minutos acima dos 2%.
  - `telemetry_redaction_override_total` - Sev 2 imediatamente; certifique-se de que as anulações tenham tickets de conformidade.
- Execute a lista de verificação de remediação de telemetria no [plano de remediação de telemetria de Nexus](./nexus-telemetry-remediation) pelo menos trimestralmente e adicione o formulário preenchido às notas de revisão de operações.

## Matriz de incidentes

| Severidade | Definição | Resposta |
|----------|------------|----------|
| 1º de setembro | Brecha de isolamento de espaço de dados, paro de liquidação >15 min ou corrupção de voto de governo. | Pagear a Nexus Primário + Engenharia de Liberação + Conformidade, congelar admissão, coletar artefatos, publicar comunicados <=60 min, RCA <=5 dias hábiles. |
| 2 de setembro | Cumprimento de SLA de backlog de pista, ponto de telemetria >30 min, rollout de manifestos falido. | Pagear a Nexus Primário + SRE, mitigar <=4 h, acompanhamento de registrador em 2 dias úteis. |
| 3 de setembro | Deriva no bloqueante (docs, alertas). | Registre-se no rastreador e planeje o arreglo dentro do sprint. |

Os tickets de incidentes devem registrar IDs de pista/espaço de dados afetados, hashes de manifestação, linha de tempo, métricas/logs de suporte e tarefas/proprietários de acompanhamento.

## Arquivo de evidências- Guarda pacotes/manifestos/exportações de telemetria bajo `artifacts/nexus/<lane>/<date>/`.
- Conserva configurações editadas + saída de `--trace-config` para cada versão.
- Adjunta minutas del consejo + decisões firmadas quando você altera alterações de configuração ou manifestação.
- Conserva snapshots semanais de Prometheus relevantes para métricas de Nexus durante 12 meses.
- Registre as edições do runbook em `docs/source/project_tracker/nexus_config_deltas/README.md` para que os auditores se espalhem quando as responsabilidades forem alteradas.

## Material relacionado

- Resumo: [Visão geral Nexus] (./nexus-overview)
- Especificação: [especificação Nexus] (./nexus-spec)
- Geometria de pistas: [modelo de pista Nexus] (./nexus-lane-model)
- Transição e calços de roteamento: [Nexus notas de transição](./nexus-transition-notes)
- Onboarding de operadores: [Sora Nexus operador onboarding](./nexus-operator-onboarding)
- Remediação de telemetria: [Plano de remediação de telemetria Nexus](./nexus-telemetry-remediation)