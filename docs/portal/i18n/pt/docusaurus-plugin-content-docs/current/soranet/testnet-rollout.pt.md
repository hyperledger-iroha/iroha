---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lançamento testnet
título: Rollout do testnet do SoraNet (SNNet-10)
sidebar_label: Implementação do testnet (SNNet-10)
descrição: Plano de ativação por fases, kit de onboarding e portões de telemetria para promoções de testnet do SoraNet.
---

:::nota Fonte canônica
Esta página reflete o plano de implementação do SNNet-10 em `docs/source/soranet/testnet_rollout_plan.md`. Mantenha ambas as cópias sincronizadas.
:::

SNNet-10 coordena a ativação em etapas do overlay de anonimato do SoraNet em toda a rede. Use este plano para traduzir o bullet do roadmap em entregáveis ​​concretos, runbooks e portões de telemetria para que cada operador entenda as expectativas antes de SoraNet virar o padrão de transporte.

## Fases de lançamento

| Fase | Cronograma (alvo) | Escopo | Artefatos obrigatórios |
|-------|-------------------|-------|---------|
| **T0 - Testnet fechado** | 4º trimestre de 2026 | 20-50 relés em >=3 ASNs ocupados por contribuidores core. | Kit de integração Testnet, conjunto de fumaça de fixação de proteção, linha de base de latência + métricas de PoW, registro de brownout drill. |
| **T1 - Publicação Beta** | 1º trimestre de 2027 | >=100 relés, rotação de guarda habilitada, ligação de saída aplicada, SDK beta padrão em SoraNet com `anon-guard-pq`. | Onboarding kit atualizado, checklist de verificação de operadores, SOP de publicação de diretório, pacote de dashboards de telemetria, relatórios de ensaio de incidentes. |
| **T2 - Padrão Mainnet** | 2º trimestre de 2027 (condicionado a completar SNNet-6/7/9) | Rede de produção padrão em SoraNet; transporta obfs/MASQUE e aplicação de catraca PQ habilitada. | Minutas de aprovação de governança, procedimento de rollback direct-only, alarmes de downgrade, relatório assinado de métricas de sucesso. |

Não há **caminho de salto** - cada fase deve entregar a telemetria e os artistas de governança da etapa anterior antes da promoção.

## Kit de onboarding do testnet

Cada operador de retransmissão recebe um pacote determinístico com os seguintes arquivos:

| Artefato | Descrição |
|----------|------------|
| `01-readme.md` | Resumo, pontos de contato e cronograma. |
| `02-checklist.md` | Checklist de pré-voo (hardware, acessibilidade da rede, política de verificação de guarda). |
| `03-config-example.toml` | Configuração mínima de relé + orquestrador SoraNet compatível com blocos de conformidade do SNNet-9, incluindo um bloco `guard_directory` que fixa o hash do ultimo guard snapshot. |
| `04-telemetry.md` | Instruções para ligar os dashboards de métricas de privacidade do SoraNet e os limites de alerta. |
| `05-incident-playbook.md` | Procedimento de resposta a brownout/downgrade com matriz de escalonamento. |
| `06-verification-report.md` | Modelo que os operadores completam e devolvem quando os testes de fumaça passam. |

Uma cópia renderizada viva em `docs/examples/soranet_testnet_operator_kit/`. Cada promoção atualiza o kit; números de versão acompanham a fase (por exemplo, `testnet-kit-vT0.1`).Para operadores de beta pública (T1), o breve conciso em `docs/source/soranet/snnet10_beta_onboarding.md` resume pré-requisitos, entregaveis de telemetria e o fluxo de envio, apontando para o kit determinístico e para os auxiliares de validação.

`cargo xtask soranet-testnet-feed` gera um feed JSON que adiciona uma janela de promoção, lista de relés, relatório de métricas, evidências de treinos e hashes de anexos referenciados pelo modelo de stage-gate. Assine os logs de perfuração e anexos com `cargo xtask soranet-testnet-drill-bundle` primeiro para que o feed registre `drill_log.signed = true`.

## Métricas de sucesso

A promoção entre fases fica fechada na seguinte telemetria, coletada por no mínimo duas semanas:

- `soranet_privacy_circuit_events_total`: 95% dos circuitos completados sem eventos de brownout ou downgrade; os 5% restantes ficam limitados pela oferta de PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: =99% dentro da janela alvo de 3 s; reportado via `soranet_privacy_throttles_total{scope="congestion"}`.
- Latência (percentil 95) por regiao: <200 ms quando os circuitos estiverem totalmente construídos, capturados via `soranet_privacy_rtt_millis{percentile="p95"}`.

Templates de dashboards e alertas vivem em `dashboard_templates/` e `alert_templates/`; espele-os no seu repositório de telemetria e adicione-os a verificações de lint do CI. Use `cargo xtask soranet-testnet-metrics` para gerar o relato relacionado à governança antes de solicitar uma promoção.

Os envios do stage-gate devem seguir `docs/source/soranet/snnet10_stage_gate_template.md`, que aponta para o formulário Markdown pronto para copiar em `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist de verificação

Os operadores devem revisar o seguinte antes de entrar em cada fase:

- [x] Retransmissão de anúncio assinado com o envelope de admissão atual.
- [x] Teste de fumaça de rotação da guarda (`tools/soranet-relay --check-rotation`) aprovado.
- [x] `guard_directory` aponta para os artefatos `GuardDirectorySnapshotV2` mais recente e `expected_directory_hash_hex` coincidem com o Committee Digest (o startup do relay registra o hash validado).
- [x] Métricas de catraca PQ (`sorafs_orchestrator_pq_ratio`) permanecem acima dos limites alvo para a fase solicitada.
- [x] Configuração de conformidade GAR correspondente à tag mais recente (ver catálogo SNNet-9).
- [x] Simulação de alarme de downgrade (desativar coletores, aguardar alerta em 5 min).
- [x] Drill PoW/DoS concluído com etapas de mitigação documentadas.

Um modelo pré-preenchido não inclui kit de integração. Os operadores submetem o relatório completo ao helpdesk de governança antes de receberem credenciais de produção.

## Governança e relatórios- **Controle de alterações:** promoções desativadas aprovacao do Conselho de Governança registradas nas minutas do conselho e anexadas à página de status.
- **Status digest:** publicar atualizações semanais resumindo contagem de relés, razão PQ, incidentes de brownout e itens de ação pendentes (armazenado em `docs/source/status/soranet_testnet_digest.md` quando a cadência comecar).
- **Reversões:** manter um plano de reversão contratado que retornará à rede para a fase anterior em 30 minutos, incluindo invalidação de DNS/guard cache e modelos de comunicação com clientes.

## Ativos de apoio

- `cargo xtask soranet-testnet-kit [--out <dir>]` materializa o kit de onboarding de `xtask/templates/soranet_testnet/` para o diretório alvo (padrão `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` avalia as métricas de sucesso SNNet-10 e emite um relatório de aprovação/reprovação estruturado para revisões de governança. Um instantâneo de exemplo vivo em `docs/examples/soranet_testnet_metrics_sample.json`.
- Templates de Grafana e Alertmanager vivem em `dashboard_templates/soranet_testnet_overview.json` e `alert_templates/soranet_testnet_rules.yml`; copie-os no repositório de telemetria ou conecte-os aos cheques de lint do CI.
- O template de comunicação de downgrade para mensagens de SDK/portal reside em `docs/source/soranet/templates/downgrade_communication_template.md`.
- Os resumos semanais de status devem usar `docs/source/status/soranet_testnet_weekly_digest.md` como forma canônica.

Pull requests devem atualizar esta página junto com qualquer mudança de arte ou telemetria para que o plano de implementação seja canônico.