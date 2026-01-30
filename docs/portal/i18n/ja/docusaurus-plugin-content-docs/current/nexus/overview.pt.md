---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-overview
title: Visao geral do Sora Nexus
description: Resumo de alto nivel da arquitetura do Iroha 3 (Sora Nexus) com apontamentos para os docs canonicos do mono-repo.
---

Nexus (Iroha 3) estende Iroha 2 com execucao multi-lane, espacos de dados escopados por governanca e ferramentas compartilhadas em cada SDK. Esta pagina espelha o novo resumo `docs/source/nexus_overview.md` no mono-repo para que leitores do portal entendam rapidamente como as pecas da arquitetura se encaixam.

## Linhas de release

- **Iroha 2** - implantacoes auto-hospedadas para consorcios ou redes privadas.
- **Iroha 3 / Sora Nexus** - a rede publica multi-lane onde operadores registram espacos de dados (DS) e herdam ferramentas compartilhadas de governanca, liquidacao e observabilidade.
- Ambas as linhas compilam do mesmo workspace (IVM + toolchain Kotodama), entao correcoes de SDK, atualizacoes de ABI e fixtures Norito permanecem portaveis. Operadores baixam o bundle `iroha3-<version>-<os>.tar.zst` para entrar no Nexus; consulte `docs/source/sora_nexus_operator_onboarding.md` para o checklist em tela cheia.

## Blocos de construcao

| Componente | Resumo | Pontos do portal |
|-----------|---------|--------------|
| Espaco de dados (DS) | Dominio de execucao/armazenamento definido pela governanca que possui uma ou mais lanes, declara conjuntos de validadores, classe de privacidade e politica de taxas + DA. | Veja [Nexus spec](./nexus-spec) para o esquema do manifesto. |
| Lane | Shard deterministico de execucao; emite compromissos que o anel global NPoS ordena. As classes de lane incluem `default_public`, `public_custom`, `private_permissioned` e `hybrid_confidential`. | O [modelo de lane](./nexus-lane-model) captura geometria, prefixos de armazenamento e retencao. |
| Plano de transicao | Identificadores placeholder, fases de roteamento e empacotamento de perfil duplo acompanham como implantacoes de lane unica evoluem para Nexus. | As [notas de transicao](./nexus-transition-notes) documentam cada fase de migracao. |
| Space Directory | Contrato de registro que armazena manifestos + versoes de DS. Operadores reconciliam entradas do catalogo com este diretorio antes de entrar. | O rastreador de diffs de manifesto vive em `docs/source/project_tracker/nexus_config_deltas/`. |
| Catalogo de lanes | A secao `[nexus]` de configuracao mapeia IDs de lane para aliases, politicas de roteamento e limiares de DA. `irohad --sora --config ... --trace-config` imprime o catalogo resolvido para auditorias. | Use `docs/source/sora_nexus_operator_onboarding.md` para o passo a passo de CLI. |
| Roteador de liquidacao | Orquestrador de transferencia XOR que conecta lanes CBDC privadas com lanes de liquidez publicas. | `docs/source/cbdc_lane_playbook.md` detalha knobs de politica e gates de telemetria. |
| Telemetria/SLOs | Dashboards + alertas em `dashboards/grafana/nexus_*.json` capturam altura de lanes, backlog de DA, latencia de liquidacao e profundidade da fila de governanca. | O [plano de remediacao de telemetria](./nexus-telemetry-remediation) detalha dashboards, alertas e evidencias de auditoria. |

## Snapshot de rollout

| Fase | Foco | Criterios de saida |
|-------|-------|---------------|
| N0 - Beta fechada | Registrar gerenciado pelo conselho (`.sora`), onboarding manual de operadores, catalogo de lanes estatico. | Manifestos de DS assinados + handoffs de governanca ensaiados. |
| N1 - Lancamento publico | Adiciona sufixos `.nexus`, leiloes, registrar self-service, cabeamento de liquidacao XOR. | Testes de sincronizacao de resolver/gateway, dashboards de reconciliacao de cobranca, exercicios de disputa em mesa. |
| N2 - Expansao | Introduz `.dao`, APIs de revenda, analitica, portal de disputas, scorecards de stewards. | Artefatos de compliance versionados, toolkit de jury de politica online, relatorios de transparencia do tesouro. |
| Gate NX-12/13/14 | Motor de compliance, dashboards de telemetria e documentacao devem sair juntos antes de pilotos com parceiros. | [Nexus overview](./nexus-overview) + [Nexus operations](./nexus-operations) publicados, dashboards ligados, motor de politica integrado. |

## Responsabilidades do operador

1. **Higiene de configuracao** - mantenha `config/config.toml` sincronizado com o catalogo publicado de lanes e dataspaces; arquive a saida `--trace-config` em cada ticket de release.
2. **Rastreamento de manifestos** - reconcilie entradas do catalogo com o bundle mais recente do Space Directory antes de entrar ou atualizar nos.
3. **Cobertura de telemetria** - exponha os dashboards `nexus_lanes.json`, `nexus_settlement.json` e os dashboards de SDK relacionados; conecte alertas ao PagerDuty e rode revisoes trimestrais conforme o plano de remediacao de telemetria.
4. **Relato de incidentes** - siga a matriz de severidade em [Nexus operations](./nexus-operations) e entregue RCAs em ate cinco dias uteis.
5. **Prontidao de governanca** - participe de votos do conselho Nexus que impactam suas lanes e ensaie instrucoes de rollback trimestralmente (rastreadas via `docs/source/project_tracker/nexus_config_deltas/`).

## Veja tambem

- Visao canonica: `docs/source/nexus_overview.md`
- Especificacao detalhada: [./nexus-spec](./nexus-spec)
- Geometria de lanes: [./nexus-lane-model](./nexus-lane-model)
- Plano de transicao: [./nexus-transition-notes](./nexus-transition-notes)
- Plano de remediacao de telemetria: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook de operacoes: [./nexus-operations](./nexus-operations)
- Guia de onboarding de operadores: `docs/source/sora_nexus_operator_onboarding.md`
