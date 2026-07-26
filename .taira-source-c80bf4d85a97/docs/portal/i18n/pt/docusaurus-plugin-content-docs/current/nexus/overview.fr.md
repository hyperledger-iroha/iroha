---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visão geral do nexo
título: Apercu de Sora Nexus
description: Resumo do alto nível da arquitetura Iroha 3 (Sora Nexus) com ponteiros para os documentos canônicos do mono-repo.
---

Nexus (Iroha 3) estende Iroha 2 com a execução multi-lane, os espaços de dados de quadros para o governo e as ferramentas compartilhadas em cada SDK. Esta página reflete o novo resumo `docs/source/nexus_overview.md` no mono-repo, para que os leitores do portal compreendam rapidamente as peças da arquitetura que incorporam.

## Linhas de versão

- **Iroha 2** - implantação de auto-heberges para consórcios ou pesquisas privadas.
- **Iroha 3 / Sora Nexus** - recursos públicos multi-lane ou operadores registrados de espaços de donativos (DS) e herdeiros de partes de governo, regulamentações e observabilidade.
- As duas linhas compiladas a partir do espaço de trabalho do meme (IVM + conjunto de ferramentas Kotodama), fornecem os SDKs corretos, as atualizações do dia ABI e os equipamentos Norito restantes portáteis. Os operadores baixaram o arquivo `iroha3-<version>-<os>.tar.zst` para reunir Nexus ; informe `docs/source/sora_nexus_operator_onboarding.md` para a lista de verificação em toda a tela.

## Blocos de construção

| Composto | Currículo | Pontos do portal |
|-----------|---------|-------------|
| Espaço de donativos (DS) | Domínio de execução/armazenagem definido pelo governo que possui uma ou mais pistas, declara conjuntos de validadores, classe de confidencialidade e política de frais + DA. | Veja [Nexus spec](./nexus-spec) para o esquema do manifesto. |
| Pista | Shard determinador de execução; emet des engagements que anneau NPoS global ordern. As classes de pista incluem `default_public`, `public_custom`, `private_permissioned` e `hybrid_confidential`. | O [modelo de pista](./nexus-lane-model) captura a geometria, os prefixos de estoque e a retenção. |
| Plano de transição | Os identificadores de espaço reservado, as fases de roteamento e um rastreador de perfil duplo de embalagem comentam as implantações mono-lane evolutivas vers Nexus. | As [notas de transição](./nexus-transition-notes) documentam cada fase de migração. |
| Diretório Espacial | Contrat registre qui stocke les manifestes + versões DS. Os operadores conciliam as entradas do catálogo com esse repertório antes de voltar. | Le suivi des diffs de manifeste vit sous `docs/source/project_tracker/nexus_config_deltas/`. |
| Catálogo de pistas | A seção de configuração `[nexus]` mapeia os IDs da pista em relação ao alias, políticas de rota e seus DA. `irohad --sora --config ... --trace-config` imprime a resolução do catálogo para auditorias. | Use `docs/source/sora_nexus_operator_onboarding.md` para as rotas CLI. |
| Roteador de regulamento | Orquestrador de transferências XOR que conecta as vias CBDC privadas às vias de liquidez pública. | `docs/source/cbdc_lane_playbook.md` detalha as regras políticas e os regulamentos de telemetria. |
| Telemetria/SLOs | Quadros de bordo + alertas sob `dashboards/grafana/nexus_*.json` capturam a arrogância das pistas, o backlog DA, a latência do regulamento e o profundor do arquivo de governo. | O [plano de remediação de telemetria](./nexus-telemetry-remediation) detalha os quadros de borda, alertas e precauções de auditoria. |

## Instantâneo de implantação

| Fase | Foco | Critérios de surtida |
|-------|-------|---------------|
| N0 - Beta fechado | Registrador gerenciado pelo conselho (`.sora`), operador de integração manual, catálogo de pistas estáticas. | Manifestos DS signes + passations de gouvernance repetes. |
| N1 - Lanceamento público | Adicione os sufixos `.nexus`, os vazios, um registrador de serviço gratuito, o cabo de regulamento XOR. | Testes de resolução/gateway de sincronização, tabelas de reconciliação de faturação, exercícios de litígio. |
| N2 - Expansão | Apresentamos `.dao`, fornecedores de APIs, análises, portais de litígios, scorecards de administradores. | Artefatos de versões conformes, kit de ferramentas do júri de política on-line, relatórios de transparência do tesouro. |
| Porta NX-12/13/14 | O motor de conformidade, os painéis de telemetria e a documentação devem ser agrupados antes dos pilotos parceiros. | [Visão geral Nexus](./nexus-overview) + [operações Nexus](./nexus-operations) publicações, cabos de painéis, motor de fusão política. |

## Responsabilidades dos operadores1. **Higiene de configuração** - gardez `config/config.toml` sincronizar com o catálogo publicado de pistas e espaços de dados; arquive a saída `--trace-config` com cada ticket de lançamento.
2. **Suivi des manifestes** - concilie as entradas do catálogo com o pacote anterior do Space Directory antes de se reunir ou de atingir o nível dos noeuds.
3. **Telemetria de cobertura** - expõe os painéis `nexus_lanes.json`, `nexus_settlement.json` e esses estão no SDK; envie alertas para PagerDuty e realize revisões trimestrais de acordo com o plano de remediação de telemetria.
4. **Sinalização de incidentes** - siga a matriz de gravidade em [operações Nexus](./nexus-operations) e coloque os RCAs em cinco dias úteis.
5. **Preparação de governo** - assista aos votos do conselho Nexus impactando suas pistas e repita as instruções de reversão neste trimestre (suivi via `docs/source/project_tracker/nexus_config_deltas/`).

## Veja também

- Apercu canônico: `docs/source/nexus_overview.md`
Especificação detalhada: [./nexus-spec] (./nexus-spec)
- Geometria das pistas: [./nexus-lane-model](./nexus-lane-model)
- Plano de transição: [./nexus-transition-notes](./nexus-transition-notes)
- Plano de remediação de telemetria: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Operações de runbook: [./nexus-operações](./nexus-operations)
- Guia do operador de integração: `docs/source/sora_nexus_operator_onboarding.md`