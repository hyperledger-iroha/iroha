---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lançamento testnet
título: Implantação do testnet SoraNet (SNNet-10)
sidebar_label: Implementação do testnet (SNNet-10)
descrição: Plano de ativação por fases, kit de integração e portões de telemetria para promoções do testnet SoraNet.
---

:::nota Fonte canônica
Esta página reflete o plano de implementação do SNNet-10 em `docs/source/soranet/testnet_rollout_plan.md`. Gardez les duas cópias alinhadas jusqu'a la retraite des docs historiques.
:::

SNNet-10 coordena a ativação por etapas da sobreposição de anonimato SoraNet em toda a rede. Utilize este plano para traduzir o roteiro em concretos, runbooks e portões de telemetria disponíveis para que cada operador compreenda as atenções antes que o SoraNet defina o transporte por padrão.

## Fases de lançamento

| Fase | Calendário (cible) | Portée | Requisito de artefatos |
|-------|-------------------|-------|---------|
| **T0 - Testnet fechado** | 4º trimestre de 2026 | 20-50 relés em >=3 ASNs operam par des contribuintes core. | Kit de teste de integração, fixação de proteção de fumaça, linha de base de latência + métricas PoW, registro de broca de brownout. |
| **T1 - Beta pública** | 1º trimestre de 2027 | >=100 relés, rotação de guarda ativa, imposição de ligação de saída, SDK betas padrão no SoraNet com `anon-guard-pq`. | Kit de integração do dia, lista de verificação do operador, SOP de publicação do diretório, pacote de painéis de telemetria, relatórios de ensaio de incidente. |
| **T2 - Mainnet por padrão** | 2º trimestre de 2027 (condição para conclusão SNNet-6/7/9) | O reseau de produção passa por padrão no SoraNet; transporta obfs/MASQUE e aplicação de ativos de catraca PQ. | Ata de governança de aprovação, procedimento de reversão somente direto, alarmes de rebaixamento, relatório de métricas de sucesso. |

Il n'y a **aucune voie de saut** - chaque phase doit livrer la telemetrie et les artefatos de governança da etapa anterior à promoção.

## Kit de integração testnet

Cada operador de relé recupera um pacote determinado com os seguintes arquivos:

| Artefato | Descrição |
|----------|------------|
| `01-readme.md` | Vista do conjunto, pontos de contato e calendário. |
| `02-checklist.md` | Lista de verificação pré-voo (hardware, reserva de acessibilidade, política de verificação da guarda). |
| `03-config-example.toml` | Configuração mínima de relé + orquestrador SoraNet alinhado aos blocos de conformidade SNNet-9, incluindo um bloco `guard_directory` que fixa o hash do último protetor de instantâneo. |
| `04-telemetry.md` | Instruções para configurar os painéis de métricas de privacidade SoraNet e seus alertas. |
| `05-incident-playbook.md` | Procedimento de resposta de brownout/downgrade com matriz de escalada. |
| `06-verification-report.md` | Modelo para que os operadores reembolsem e devolvam uma vez que os testes de fumaça são válidos. |

Uma cópia está disponível em `docs/examples/soranet_testnet_operator_kit/`. Promoção Chaque rafraichit le kit; os números de versão seguintes à fase (por exemplo, `testnet-kit-vT0.1`).Para os operadores beta público (T1), o breve resumo em `docs/source/soranet/snnet10_beta_onboarding.md` resume os pré-requisitos, os recursos de telemetria e o fluxo de trabalho de envio de todos os recursos para o kit determinado e os auxiliares de validação.

`cargo xtask soranet-testnet-feed` gera o feed JSON que agrega a janela de promoção, a lista de relés, o relacionamento de métricas, as previsões de exercícios e os hashes de peças conjuntas referenciadas pelo modelo stage-gate. Assine as toras de perfuração e as peças juntas com `cargo xtask soranet-testnet-drill-bundle` antes de registrar o feed `drill_log.signed = true`.

## Métricas de sucesso

A promoção entre fases está condicionada à seguinte telemetria, coletada durante menos de duas semanas:

- `soranet_privacy_circuit_events_total`: 95% dos circuitos terminam sem quedas de energia e downgrade; os 5% restantes são limites para a oferta PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: =99% na janela de segurança de 3 s; relato via `soranet_privacy_throttles_total{scope="congestion"}`.
- Latência (percentil 95e) por região: <200 ms antes dos circuitos serem concluídos, capturados via `soranet_privacy_rtt_millis{percentile="p95"}`.

Os modelos de painéis e alertas vivem em `dashboard_templates/` e `alert_templates/`; recupere-os em seu repositório de telemetria e adicione-os às verificações de lint CI. Utilize `cargo xtask soranet-testnet-metrics` para gerar o relacionamento orientado à governança antes de exigir a promoção.

Os soumissions stage-gate devem seguir `docs/source/soranet/snnet10_stage_gate_template.md`, que enviam o formulário Markdown para uma copiadora sob `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist de verificação

Os operadores devem validar os seguintes pontos antes de entrar em cada fase:

- ✅ Retransmita o sinal do anúncio com o envelope de admissão atualizado.
- ✅ Teste de fumaça de rotação da guarda (`tools/soranet-relay --check-rotation`) válido.
- ✅ `guard_directory` aponta para o último artefato `GuardDirectorySnapshotV2` e `expected_directory_hash_hex` correspondem ao resumo do comite (o descasque do relé registra o hash válido).
- ✅ As métricas da catraca PQ (`sorafs_orchestrator_pq_ratio`) permanecem acima dos seus cabos para a fita exigida.
- ✅ A configuração de conformidade GAR corresponde à tag mais recente (veja o catálogo SNNet-9).
- ✅ Simulação de downgrade de alarme (desativa os coletores, atende um alerta por 5 min).
- ✅ Drill PoW/DoS executado com etapas de mitigação documentadas.

Um modelo pré-rempli está incluído no kit de integração. Os operadores realizam o relacionamento completo com a governança do helpdesk antes de receberem as credenciais de produção.

## Governança e relatórios- **Controle de alterações:** as promoções exigem a aprovação do Conselho de Governança registradas nas atas do conselho e juntas na página de status.
- **Status digest:** publicou as misses a jour hebdomadaires resumindo o nome dos relés, a proporção PQ, os incidentes de brownout e os itens de ação em atenção (stocke em `docs/source/status/soranet_testnet_digest.md` une fois la cadence lancee).
- **Reversões:** manter um plano de reversão assinando o que resta na fase anterior em 30 minutos, incluindo a invalidação de DNS/guard cache e os modelos de comunicação do cliente.

## Ativos de suporte

- `cargo xtask soranet-testnet-kit [--out <dir>]` materializa o kit de integração a partir de `xtask/templates/soranet_testnet/` no repertório disponível (por padrão `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` avalia as métricas de sucesso SNNet-10 e emet uma estrutura de relacionamento aprovação/reprovação adaptada às revistas de governança. Um instantâneo de exemplo em `docs/examples/soranet_testnet_metrics_sample.json`.
- Os modelos Grafana e Alertmanager vivem sob `dashboard_templates/soranet_testnet_overview.json` e `alert_templates/soranet_testnet_rules.yml`; copie-os em seu repositório de telemetria ou ramifica-os nas verificações de lint CI.
- O modelo de downgrade de comunicação para as mensagens SDK/portal reside em `docs/source/soranet/templates/downgrade_communication_template.md`.
- Les digests de status hebdomadaires devem utilizar `docs/source/status/soranet_testnet_weekly_digest.md` como forma canônica.

As solicitações pull devem ser enviadas para esta página no dia com todas as alterações de artefatos ou telemetria após o plano de implementação ser canônico.