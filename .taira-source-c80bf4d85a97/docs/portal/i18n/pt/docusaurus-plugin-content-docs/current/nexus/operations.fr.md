---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nexo
título: Operações de runbook Nexus
descrição: Currículo operacional pronto para o terreno do operador de fluxo de trabalho Nexus, refletivo `docs/source/nexus_operations.md`.
---

Utilize esta página como companheiro de referência rápida de `docs/source/nexus_operations.md`. Ela condensa a lista de verificação operacional, os pontos de controle de gerenciamento de alterações e as exigências de cobertura telefônicas que os operadores Nexus devem seguir.

## Checklist do ciclo de vida

| Etapa | Ações | Preúves |
|-------|--------|----------|
| Pré-vol | Verifique os hash/assinaturas de lançamento, confirme `profile = "iroha3"` e prepare os modelos de configuração. | Sortie de `scripts/select_release_profile.py`, diário de checksum, pacote de manifestos assinado. |
| Alinhamento do catálogo | Mettre a jour le catalog `[nexus]`, la politique de rouge et les seusils DA selon le manifeste émis par le conseil, puis capturer `--trace-config`. | Sortie `irohad --sora --config ... --trace-config` estocado com o ticket de embarque. |
| Fumaça e corte | Lancer `irohad --sora --config ... --trace-config`, executa a fumaça do CLI (`FindNetworkStatus`), valida as exportações de telefonia e exige a admissão. | Log de teste de fumaça + confirmação Alertmanager. |
| Regime estável | Painéis/alertas de monitoramento fazem o tour das chaves de acordo com a cadência de governança e sincronizam configurações/runbooks quando os manifestos são alterados. | Minutos de revisão trimestral, captura painéis, IDs de tickets de rotação. |

O detalhe de integração (substituição de chaves, modelos de rota, etapas de perfil de lançamento) está localizado em `docs/source/sora_nexus_operator_onboarding.md`.

## Gerenciamento de alteração

1. **Mises no dia do lançamento** - siga os anúncios em `status.md`/`roadmap.md` ; junte-se à lista de verificação de integração em cada PR de lançamento.
2. **Alterações de manifestos de pista** - verifique os pacotes assinados no Space Directory e os arquivadores sob `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuração** - toda alteração de `config/config.toml` requer um ticket referente à pista/espaço de dados. Mantenha uma cópia eliminada da configuração efetiva ao ingressar/atualizar noeuds.
4. **Exercícios de reversão** - repete trimestralmente os procedimentos stop/restore/smoke ; envie os resultados em `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprovações em conformidade** - as faixas privadas/CBDC devem obter uma feu vert conformidade antes de modificar a política DA ou os botões de redação de televisão (veja `docs/source/cbdc_lane_playbook.md`).

## Telemetria e SLOs

- Painéis: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, além de vistas específicas do SDK (por exemplo, `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` e regras de transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métricas de vigilância:
  - `nexus_lane_height{lane_id}` - alerta em caso de ausência de progressão pendente de três slots.
  - `nexus_da_backlog_chunks{lane_id}` - alerter au-dessus des seusils par lane (por padrão 64 público / 8 privado).
  - `nexus_settlement_latency_seconds{lane_id}` - alerta quando o P99 ultrapassa 900 ms (público) ou 1200 ms (privado).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerta se a taxa de erro em 5 minutos ultrapassar 2%.
  - `telemetry_redaction_override_total` - Sev 2 imediatamente; garantir a conformidade dos tickets para as substituições.
- Executar a lista de verificação de remediação telefônica no [plano de remediação telefônica Nexus](./nexus-telemetry-remediation) pelo menos trimestralmente e juntar o formulário preenchido às notas de operações de revista.

## Matriz de incidente

| Gravidade | Definição | Resposta |
|----------|------------|----------|
| 1º de setembro | Brèche d'isolation data-space, prisão de liquidação >15 min, ou corrupção de voto de governo. | Alerta Nexus Primário + Engenharia de Liberação + Conformidade, permite a admissão, coleta os artefatos, publica comunicações <=60 min, RCA <=5 dias abertos. |
| 2 de setembro | Violação de SLA de backlog de pista, ângulo morto de télémétrie >30 min, lançamento de manifesto ecoado. | Alerta Nexus Primário + SRE, atenuador <=4 h, colocado de lado durante 2 dias abertos. |
| 3 de setembro | Derive non bloquante (docs, alertes). | Registre-se no rastreador, planeje a correção no sprint. |

Os tickets de incidente devem registrar os IDs de pista/espaço de dados afetados, os hashes de manifesto, a cronologia, as métricas/logs de suporte e as tags/proprietários de acompanhamento.

## Arquivo de testes- Armazenar pacotes/manifestos/exportações de télémétrie sob `artifacts/nexus/<lane>/<date>/`.
- Conservar configurações expurgadas + saída `--trace-config` para cada liberação.
- Junte minutos de conselho + decisões assinadas quando alterações de configuração ou manifesto forem aplicadas.
- Guarde os instantâneos Prometheus hebdomadaires des métriques Nexus pendentes 12 meses.
- Registre as modificações do runbook em `docs/source/project_tracker/nexus_config_deltas/README.md` para que os auditores sejam informados quando suas responsabilidades forem alteradas.

## Material lié

- Visão geral do conjunto: [Visão geral Nexus] (./nexus-overview)
Especificação: [especificação Nexus] (./nexus-spec)
- Geometria das pistas: [modelo de pista Nexus] (./nexus-lane-model)
- Transição e calços de rota: [notas de transição Nexus] (./nexus-transition-notes)
- Operador de integração: [integração do operador Sora Nexus] (./nexus-operator-onboarding)
- Remédiation télémétrie : [Nexus plano de remediação de telemetria](./nexus-telemetry-remediation)