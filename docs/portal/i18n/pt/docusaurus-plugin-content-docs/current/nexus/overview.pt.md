---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visão geral do nexo
título: Visão geral do Sora Nexus
description: Resumo de alto nível da arquitetura do Iroha 3 (Sora Nexus) com apontamentos para os documentos canônicos do mono-repo.
---

Nexus (Iroha 3) estende Iroha 2 com execução multi-lane, espaços de dados escapados por governança e ferramentas compartilhadas em cada SDK. Esta página reflete o novo resumo `docs/source/nexus_overview.md` no mono-repo para que os leitores do portal entendam rapidamente como as peças da arquitetura se encaixam.

## Linhas de lançamento

- **Iroha 2** - implantações auto-hospedadas para consórcios ou redes privadas.
- **Iroha 3 / Sora Nexus** - uma rede pública multi-lane onde operadores registram espaços de dados (DS) e herdam ferramentas compartilhadas de governança, liquidação e observabilidade.
- Ambas as linhas compilam do mesmo workspace (IVM + toolchain Kotodama), então correções de SDK, atualizações de ABI e fixtures Norito permanecem portáteis. Operadores baixam o pacote `iroha3-<version>-<os>.tar.zst` para entrar no Nexus; consulte `docs/source/sora_nexus_operator_onboarding.md` para a lista de verificação em tela cheia.

## Blocos de construção

| Componente | Resumo | Pontos do portal |
|-----------|---------|-------------|
| Espaço de dados (DS) | Domínio de execução/armazenamento definido pela governança que possui uma ou mais vias, conjuntos de declaração de validadores, classe de privacidade e política de taxas + DA. | Veja [Nexus spec](./nexus-spec) para o esquema do manifesto. |
| Pista | Shard determinístico de execução; emitir compromissos que o NPoS global ordena. As classes de pista incluem `default_public`, `public_custom`, `private_permissioned` e `hybrid_confidential`. | O [modelo de pista](./nexus-lane-model) captura geometria, prefixos de armazenamento e retenção. |
| Plano de transição | Identificadores placeholder, fases de roteamento e empacotamento de perfil duplo acompanham como implantações de pista única evoluem para Nexus. | As [notas de transição](./nexus-transition-notes) documentam cada fase de migração. |
| Diretório Espacial | Contrato de registro que armazena manifestos + versoes do DS. Operadores reconciliam entradas do catálogo com este diretório antes de entrar. | O rastreador de diferenças de manifesto vive em `docs/source/project_tracker/nexus_config_deltas/`. |
| Catálogo de pistas | A seção `[nexus]` de configuração do mapa de IDs de pista para aliases, políticas de roteamento e limites de DA. `irohad --sora --config ... --trace-config` imprime o catálogo resolvido para auditorias. | Use `docs/source/sora_nexus_operator_onboarding.md` para passar a etapa da CLI. |
| Roteador de liquidação | Orquestrador de transferência XOR que conecta vias CBDC privadas com vias de liquidez pública. | `docs/source/cbdc_lane_playbook.md` detalhes botões de política e portões de telemetria. |
| Telemetria/SLOs | Dashboards + alertas em `dashboards/grafana/nexus_*.json` capturam altura de pistas, backlog de DA, latência de liquidação e profundidade da fila de governança. | O [plano de remediação de telemetria](./nexus-telemetry-remediation) detalha dashboards, alertas e evidências de auditorias. |

## Instantâneo do lançamento

| Fase | Foco | Critérios de disseda |
|-------|-------|---------------|
| N0 - Beta fechado | Registrador gerenciado pelo conselho (`.sora`), manual de integração de operadores, catálogo de pistas estáticas. | Manifestos de DS assinados + handoffs de governança ensaiados. |
| N1 - Lancamento público | Adiciona sufixos `.nexus`, leiloes, registrador self-service, cabeamento de liquidação XOR. | Testes de sincronização de resolvedor/gateway, dashboards de reconciliação de cobranca, exercícios de disputa em mesa. |
| N2 - Expansão | Apresentamos `.dao`, APIs de revenda, análise, portal de disputas, scorecards de stewards. | Artefatos de conformidade versionados, kit de ferramentas do júri de política online, relatórios de transparência do tesouro. |
| Portão NX-12/13/14 | Motor de compliance, dashboards de telemetria e documentação devem sair juntos antes dos pilotos com parceiros. | [Visão geral do Nexus](./nexus-overview) + [Operações do Nexus](./nexus-operations) publicados, painéis ligados, motor de política integrado. |

## Responsabilidades do operador1. **Higiene de configuração** - mantenha `config/config.toml` sincronizado com o catálogo publicado de pistas e espaços de dados; arquive a saida `--trace-config` em cada ticket de lançamento.
2. **Rastreamento de manifestos** - reconcilie as entradas do catálogo com o pacote mais recente do Space Directory antes de entrar ou atualizar nos.
3. **Cobertura de telemetria** - exponha os dashboards `nexus_lanes.json`, `nexus_settlement.json` e os dashboards de SDK relacionados; conecte alertas ao PagerDuty e rode revisões trimestrais conforme o plano de remediação de telemetria.
4. **Relato de incidentes** - siga a matriz de severidade em [operações Nexus](./nexus-operations) e entregue RCAs em até cinco dias úteis.
5. **Prontidão de governança** - participe dos votos do conselho Nexus que impactam suas faixas e ensaiam instruções de reversão trimestralmente (rastreadas via `docs/source/project_tracker/nexus_config_deltas/`).

## Veja também

- Visão canônica: `docs/source/nexus_overview.md`
- Especificação específica: [./nexus-spec](./nexus-spec)
- Geometria das pistas: [./nexus-lane-model](./nexus-lane-model)
- Plano de transição: [./nexus-transition-notes](./nexus-transition-notes)
- Plano de remediação de telemetria: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook de operações: [./nexus-operações](./nexus-operations)
- Guia de integração de operadores: `docs/source/sora_nexus_operator_onboarding.md`