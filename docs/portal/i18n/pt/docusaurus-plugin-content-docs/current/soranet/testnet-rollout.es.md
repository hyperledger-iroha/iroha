---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lançamento testnet
título: Despliegue de testnet de SoraNet (SNNet-10)
sidebar_label: Despliegue de testnet (SNNet-10)
descrição: Plano de ativação por fases, kit de onboarding e portões de telemetria para promoções de testnet de SoraNet.
---

:::nota Fonte canônica
Esta página reflete o plano de implementação do SNNet-10 em `docs/source/soranet/testnet_rollout_plan.md`. Mantenha ambas as cópias alinhadas até que os documentos herdados sejam retirados.
:::

SNNet-10 coordena a ativação escalonada da sobreposição de anonimato do SoraNet em toda a rede. Usamos este plano para traduzir o marcador do roteiro em entregas específicas, runbooks e portões de telemetria para que cada operador compreenda as expectativas antes que a SoraNet envie o transporte por defeito.

## Fases de lançamento

| Fase | Cronograma (objetivo) | Alcance | Artefatos necessários |
|-------|-------------------|-------|---------|
| **T0 - Testnet cerrado** | 4º trimestre de 2026 | 20-50 relés en >=3 ASNs ocupados por contribuintes core. | Kit de onboarding de testnet, conjunto de fumaça de pinning de proteção, linha de base de latência + métricas de PoW, registro de broca de brownout. |
| **T1 - Publicação beta** | 1º trimestre de 2027 | >=100 relés, rotação de guarda habilitada, ligação de saída aplicada, SDK betas com defeito em SoraNet com `anon-guard-pq`. | Kit de onboarding atualizado, checklist de verificação de operadoras, SOP de publicação de diretório, pacote de dashboards de telemetria, relatórios de ensaio de incidentes. |
| **T2 - Rede principal por defeito** | 2º trimestre de 2027 (condicionado a completar SNNet-6/7/9) | A rede de produção por defeito no SoraNet; transporta obfs/MASQUE e aplicação de catraca PQ habilitada. | Minutas de aprovação de governança, procedimento de reversão somente direto, alarmes de downgrade, relatório firmado de métricas de saída. |

Não há nenhuma **rota de salto**: cada fase deve entregar a telemetria e os artefatos de governança da etapa anterior antes da promoção.

## Kit de onboarding de testnet

Cada operador de retransmissão recebe um pacote determinístico com os seguintes arquivos:

| Artefato | Descrição |
|----------|------------|
| `01-readme.md` | Resumo, pontos de contato e cronograma. |
| `02-checklist.md` | Checklist de pré-voo (hardware, acessibilidade de rede, verificação de política de guarda). |
| `03-config-example.toml` | Configuração mínima de relé + orquestrador SoraNet alineada com blocos de conformidade SNNet-9, incluindo um bloco `guard_directory` que fixa o hash do snapshot de guarda mais recente. |
| `04-telemetry.md` | Instruções para conectar os painéis de métricas de privacidade do SoraNet e os limites de alertas. |
| `05-incident-playbook.md` | Procedimento de resposta a brownout/downgrade com matriz de escalamento. |
| `06-verification-report.md` | Plantilla que os operadores completam e devolvem uma vez que passam os testes de fumaça. |

Uma cópia renderizada vive em `docs/examples/soranet_testnet_operator_kit/`. Cada promoção refresca o kit; os números de versão seguem a fase (por exemplo, `testnet-kit-vT0.1`).Para operadores de beta pública (T1), o resumo conciso em `docs/source/soranet/snnet10_beta_onboarding.md` resume pré-requisitos, entregas de telemetria e fluxo de envio, enquanto se dirige ao kit determinístico e aos auxiliares de validação.

`cargo xtask soranet-testnet-feed` gera o feed JSON que adiciona a janela de promoção, lista de relés, relatório de métricas, evidência de treinos e hashes de adjuntos referenciados pela planta de estágio-gate. Firme os logs de perfuração e adjuntos com `cargo xtask soranet-testnet-drill-bundle` primeiro para que o registro de feed `drill_log.signed = true`.

## Métricas de saída

A promoção entre fases é habilitada com a seguinte telemetria, selecionada por um mínimo de duas semanas:

- `soranet_privacy_circuit_events_total`: 95% dos circuitos completados sem eventos de queda de energia ou downgrade; Os 5% restantes são limitados pelo fornecimento PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: =99% dentro da janela objetivo de 3 s; reportado via `soranet_privacy_throttles_total{scope="congestion"}`.
- Latência (percentil 95) por região: <200 ms uma vez que os circuitos foram completamente armados, capturados via `soranet_privacy_rtt_millis{percentile="p95"}`.

Os modelos de painéis e alertas vivem em `dashboard_templates/` e `alert_templates/`; reflita em seu repositório de telemetria e adicione verificações de lint de CI. Usa `cargo xtask soranet-testnet-metrics` para gerar o relatório de cara a governança antes de pedir promoção.

As apresentações do stage-gate devem seguir `docs/source/soranet/snnet10_stage_gate_template.md`, que segue o formulário da lista Markdown para copiar abaixo `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist de verificação

Os operadores devem aprovar o seguinte antes de entrar em cada fase:

- ✅ Retransmitir anúncio firmado com o envelope de admissão real.
- ✅Teste de fumaça de guarda rotação (`tools/soranet-relay --check-rotation`) aprovado.
- ✅ `guard_directory` apunta al artefato `GuardDirectorySnapshotV2` mas recente e `expected_directory_hash_hex` coincidem com o resumo do comite (a inicialização do relé registra o hash validado).
- ✅ As métricas da catraca PQ (`sorafs_orchestrator_pq_ratio`) são mantidas por cima dos limites objetivos para a etapa solicitada.
- ✅ A configuração de conformidade GAR coincide com a tag mais recente (ver o catálogo SNNet-9).
- ✅ Simulação de alarme de downgrade (desabilitar coletores, aguardar alerta em 5 min).
- ✅ Drill PoW/DoS executado com etapas de mitigação documentadas.

Uma planta pré-preenchida está incluída no kit de integração. Os operadores enviam o relatório completo à mesa de ajuda à governação antes de receberem credenciais de produção.

## Governança e relatórios- **Controle de mudanças:** as promoções exigem aprovação do Conselho de Governança, registradas nos minutos do conselho e adjuntas à página de estado.
- **Resumo de estado:** publica atualizações semanais que resumem conteúdo de relés, relação PQ, incidentes de quedas de energia e itens de ação pendentes (se armazenam em `docs/source/status/soranet_testnet_digest.md` quando inicia a cadência).
- **Reversões:** manter um plano de reversão firmado que retorna à fase anterior em 30 minutos, incluindo invalidação de DNS/guard cache e falhas de comunicação com clientes.

## Ativos de suporte

- `cargo xtask soranet-testnet-kit [--out <dir>]` materializa o kit de integração de `xtask/templates/soranet_testnet/` para o diretório de destino (por defeito `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` avalia as métricas de saída SNNet-10 e emite um relatório estruturado de aprovação/reprovação adequado para revisões de governança. Um instantâneo de exemplo vivo em `docs/examples/soranet_testnet_metrics_sample.json`.
- As plantas de Grafana e Alertmanager vivem em `dashboard_templates/soranet_testnet_overview.json` e `alert_templates/soranet_testnet_rules.yml`; copia-as em seu repositório de telemetria ou conecta-as a verificações de lint em CI.
- A planta de comunicação de downgrade para mensagens do SDK/portal reside em `docs/source/soranet/templates/downgrade_communication_template.md`.
- Los digests semanales de estado devem usar `docs/source/status/soranet_testnet_weekly_digest.md` como forma canônica.

Os pull requests devem atualizar esta página junto com qualquer mudança de artefato ou telemetria para que o plano de implementação seja mantido canônico.