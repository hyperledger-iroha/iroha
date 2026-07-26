---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visão geral do nexo
título: Resumo de Sora Nexus
description: Resumo de alto nível da arquitetura de Iroha 3 (Sora Nexus) com links para os documentos canônicos do mono-repo.
---

Nexus (Iroha 3) amplia Iroha 2 com execução multi-lane, espaços de dados acotados por governo e ferramentas compartidas em cada SDK. Esta página reflete o novo resumo `docs/source/nexus_overview.md` do mono-repo para que os leitores do portal entendam rapidamente como encajan as peças da arquitetura.

## Linhas de lançamento

- **Iroha 2** - despliegues autoalojados para consórcios ou redes privadas.
- **Iroha 3 / Sora Nexus** - a rede pública multi-lane onde os operadores registram espaços de dados (DS) e herramientas compartilhadas de governança, liquidação e observabilidade.
- Ambas as linhas são compiladas a partir do mesmo espaço de trabalho (IVM + conjunto de ferramentas de Kotodama), para que as correções do SDK, as atualizações da ABI e os fixtures Norito sejam portáteis. Os operadores baixaram o pacote `iroha3-<version>-<os>.tar.zst` para unir a Nexus; consulte `docs/source/sora_nexus_operator_onboarding.md` para obter a lista de verificação na tela completa.

## Blocos de construção

| Componente | Resumo | Ganchos do portal |
|-----------|---------|-------------|
| Espaço de dados (DS) | Domínio de ejecução/almacenamiento definido por gobernanza que apresenta uma ou mais vias, declara conjuntos de validadores, classe de privacidad e política de tarifas + DA. | Consulte [Nexus spec](./nexus-spec) para o esquema da manifestação. |
| Pista | Fragmento determinista de execução; emite compromissos que ordenam o anel NPoS global. As classes de pista incluem `default_public`, `public_custom`, `private_permissioned` e `hybrid_confidential`. | El [modelo de lane](./nexus-lane-model) geometria de captura, preferências de armazenamento e retenção. |
| Plano de transição | Identificadores placeholder, fases de enrutamiento e empaquetados de perfil duplo seguem como os despliegues de uma pista solo evolucionan hacia Nexus. | As [notas de transição](./nexus-transition-notes) documentam cada fase da migração. |
| Diretório de espaços | Contrato de registro que armazena manifestos + versões de DS. Os operadores conciliam as entradas do catálogo com este diretório antes de se unirem. | O rastreador de diferenças de manifestações vive em `docs/source/project_tracker/nexus_config_deltas/`. |
| Catálogo de pistas | Seção `[nexus]` de configuração que atribui IDs de pista a alias, políticas de enrutamiento e umbrales DA. `irohad --sora --config ... --trace-config` imprime o catálogo resultante para auditórios. | Usa `docs/source/sora_nexus_operator_onboarding.md` para o retorno de CLI. |
| Roteador de liquidação | Orquestrador de transferências XOR que conecta canais CBDC privados com canais de liquidez pública. | `docs/source/cbdc_lane_playbook.md` detalha os botões políticos e computadores de telemetria. |
| Telemetria/SLOs | Painéis + alertas abaixo `dashboards/grafana/nexus_*.json` capturam altura de pistas, backlog DA, latência de liquidação e profundidade da cola de governo. | O [plano de remediação de telemetria](./nexus-telemetry-remediation) detalha os painéis, alertas e evidências de auditoria. |

## Instantanea despliegue

| Fase | Enfoque | Critérios de saída |
|-------|-------|---------------|
| N0 - Beta cerrada | Registrador gerenciado pelo consejo (`.sora`), manual de incorporação de operadores, catálogo de pistas estáticas. | Manifestos DS firmados + traspasos de gobernanza ensayados. |
| N1 - Lançamento público | Anade sufijos `.nexus`, subastas, registrador de autoservicio, cabo de liquidação XOR. | Testes de sincronização de resolvedores/gateways, painéis de reconciliação de fatura, simulacros de disputas. |
| N2 - Expansão | Introduzir `.dao`, APIs de receita, análise, portal de disputas, scorecards de stewards. | Artefatos de cumprimento versionados, kit de ferramentas de jurado de política on-line, relatórios de transparência do tesouro. |
| Porta NX-12/13/14 | O motor de cumprimento, painéis de telemetria e documentação devem ser exibidos juntos antes de pilotos com sócios. | [Visão geral do Nexus](./nexus-overview) + [Operações do Nexus](./nexus-operations) publicados, painéis conectados, motor de política fusionado. |

## Responsabilidades do operador1. **Higiene de configuração** - manter `config/config.toml` sincronizado com o catálogo publicado de pistas e espaços de dados; arquive a saída de `--trace-config` com cada ticket de liberação.
2. **Seguimento de manifestações** - concilia as entradas do catálogo com o pacote mais recente do Space Directory antes de unir ou atualizar nós.
3. **Cobertura de telemetria** - exponha os painéis `nexus_lanes.json`, `nexus_settlement.json` e os painéis relacionados ao SDK; conecta alertas ao PagerDuty e executa revisões trimestrais após o plano de remediação de telemetria.
4. **Relatório de incidentes** - siga a matriz de severidade em [operações Nexus](./nexus-operations) e apresente RCAs dentro de cinco dias úteis.
5. **Preparação de governança** - assista às votações do conselho Nexus que impactam suas pistas e ensaya instruções de reversão trimestralmente (seguido em `docs/source/project_tracker/nexus_config_deltas/`).

## Ver também

- Resumo canônico: `docs/source/nexus_overview.md`
- Especificação detalhada: [./nexus-spec](./nexus-spec)
- Geometria das pistas: [./nexus-lane-model](./nexus-lane-model)
- Plano de transição: [./nexus-transition-notes](./nexus-transition-notes)
- Plano de remediação de telemetria: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook de operações: [./nexus-operações](./nexus-operations)
- Guia de integração de operadores: `docs/source/sora_nexus_operator_onboarding.md`