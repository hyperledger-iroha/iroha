---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9732074c004f652fe38ebf391ea48585fe05b2db42433d2c2c99e4122c837b7
source_last_modified: "2025-11-21T14:26:41.783942+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: testnet-rollout
title: Rollout do testnet do SoraNet (SNNet-10)
sidebar_label: Rollout do testnet (SNNet-10)
description: Plano de ativacao por fases, kit de onboarding e gates de telemetria para promocoes de testnet do SoraNet.
---

:::note Fonte canonica
Esta pagina espelha o plano de rollout SNNet-10 em `docs/source/soranet/testnet_rollout_plan.md`. Mantenha ambas as copias sincronizadas.
:::

SNNet-10 coordena a ativacao em etapas do overlay de anonimato do SoraNet em toda a rede. Use este plano para traduzir o bullet do roadmap em deliverables concretos, runbooks e gates de telemetria para que cada operator entenda as expectativas antes de SoraNet virar o transporte padrao.

## Fases de lancamento

| Fase | Cronograma (alvo) | Escopo | Artefatos obrigatorios |
|-------|-------------------|-------|--------------------|
| **T0 - Testnet fechado** | Q4 2026 | 20-50 relays em >=3 ASNs operados por contribuidores core. | Testnet onboarding kit, smoke suite de guard pinning, baseline de latencia + metricas de PoW, log de brownout drill. |
| **T1 - Beta publica** | Q1 2027 | >=100 relays, guard rotation habilitada, exit bonding enforced, SDK betas padrao em SoraNet com `anon-guard-pq`. | Onboarding kit atualizado, checklist de verificacao de operadores, SOP de publicacao de directory, pacote de dashboards de telemetria, relatorios de rehearsal de incidentes. |
| **T2 - Mainnet padrao** | Q2 2027 (condicionado a completar SNNet-6/7/9) | Rede de producao padrao em SoraNet; transports obfs/MASQUE e enforcement de PQ ratchet habilitados. | Minutas de aprovacao de governance, procedimento de rollback direct-only, alarmes de downgrade, relatorio assinado de metricas de sucesso. |

Nao ha **caminho de salto** - cada fase deve entregar a telemetria e os artefatos de governance da etapa anterior antes da promocao.

## Kit de onboarding do testnet

Cada operador de relay recebe um pacote deterministico com os seguintes arquivos:

| Artefato | Descricao |
|----------|-------------|
| `01-readme.md` | Resumo, pontos de contato e cronograma. |
| `02-checklist.md` | Checklist de pre-flight (hardware, network reachability, verificacao de guard policy). |
| `03-config-example.toml` | Configuracao minima de relay + orchestrator SoraNet alinhada aos compliance blocks do SNNet-9, incluindo um bloco `guard_directory` que fixa o hash do ultimo guard snapshot. |
| `04-telemetry.md` | Instrucoes para ligar os dashboards de privacy metrics do SoraNet e os thresholds de alerta. |
| `05-incident-playbook.md` | Procedimento de resposta a brownout/downgrade com matriz de escalonamento. |
| `06-verification-report.md` | Template que os operadores completam e devolvem quando os smoke tests passam. |

Uma copia renderizada vive em `docs/examples/soranet_testnet_operator_kit/`. Cada promocao atualiza o kit; numeros de versao acompanham a fase (por exemplo, `testnet-kit-vT0.1`).

Para operadores de beta publica (T1), o brief conciso em `docs/source/soranet/snnet10_beta_onboarding.md` resume prerequisitos, entregaveis de telemetria e o fluxo de envio, apontando para o kit deterministico e para os helpers de validacao.

`cargo xtask soranet-testnet-feed` gera o feed JSON que agrega a janela de promocao, roster de relays, relatorio de metricas, evidencias de drills e hashes de anexos referenciados pelo template de stage-gate. Assine os drill logs e anexos com `cargo xtask soranet-testnet-drill-bundle` primeiro para que o feed registre `drill_log.signed = true`.

## Metricas de sucesso

A promocao entre fases fica gated na seguinte telemetria, coletada por no minimo duas semanas:

- `soranet_privacy_circuit_events_total`: 95% dos circuits completam sem eventos de brownout ou downgrade; os 5% restantes ficam limitados pela oferta de PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: <1% das fetch sessions por dia acionam brownout fora de drills planejados.
- `soranet_privacy_gar_reports_total`: variancia dentro de +/-10% do mix esperado de categorias GAR; picos devem ser explicados por updates de policy aprovados.
- Taxa de sucesso de tickets PoW: >=99% dentro da janela alvo de 3 s; reportada via `soranet_privacy_throttles_total{scope="congestion"}`.
- Latencia (95th percentile) por regiao: <200 ms quando circuits estiverem totalmente construidos, capturada via `soranet_privacy_rtt_millis{percentile="p95"}`.

Templates de dashboards e alertas vivem em `dashboard_templates/` e `alert_templates/`; espelhe-os no seu repositorio de telemetria e adicione-os a checks de lint do CI. Use `cargo xtask soranet-testnet-metrics` para gerar o relatorio voltado a governance antes de solicitar a promocao.

Submissoes de stage-gate devem seguir `docs/source/soranet/snnet10_stage_gate_template.md`, que aponta para o formulario Markdown pronto para copiar em `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist de verificacao

Os operadores devem assinar o seguinte antes de entrar em cada fase:

- [x] Relay advert assinado com o admission envelope atual.
- [x] Guard rotation smoke test (`tools/soranet-relay --check-rotation`) aprovado.
- [x] `guard_directory` aponta para o artefato `GuardDirectorySnapshotV2` mais recente e `expected_directory_hash_hex` coincide com o committee digest (o startup do relay registra o hash validado).
- [x] Metricas de PQ ratchet (`sorafs_orchestrator_pq_ratio`) permanecem acima dos thresholds alvo para a fase solicitada.
- [x] Config de compliance GAR corresponde ao tag mais recente (ver catalogo SNNet-9).
- [x] Simulacao de alarme de downgrade (desativar collectors, esperar alerta em 5 min).
- [x] Drill PoW/DoS executado com etapas de mitigacao documentadas.

Um template pre-preenchido esta incluido no kit de onboarding. Os operadores submetem o relatorio completo ao helpdesk de governance antes de receber credenciais de producao.

## Governance e reporting

- **Change control:** promocoes exigem aprovacao do Governance Council registrada nas minutes do conselho e anexada a pagina de status.
- **Status digest:** publicar atualizacoes semanais resumindo contagem de relays, ratio PQ, incidentes de brownout e action items pendentes (armazenado em `docs/source/status/soranet_testnet_digest.md` quando a cadencia comecar).
- **Rollbacks:** manter um plano de rollback assinado que retorne a rede para a fase anterior em 30 minutos, incluindo invalidacao de DNS/guard cache e templates de comunicacao com clients.

## Supporting assets

- `cargo xtask soranet-testnet-kit [--out <dir>]` materializa o kit de onboarding de `xtask/templates/soranet_testnet/` para o diretorio alvo (default `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` avalia as metricas de sucesso SNNet-10 e emite um relatorio pass/fail estruturado para revisoes de governance. Um snapshot de exemplo vive em `docs/examples/soranet_testnet_metrics_sample.json`.
- Templates de Grafana e Alertmanager vivem em `dashboard_templates/soranet_testnet_overview.json` e `alert_templates/soranet_testnet_rules.yml`; copie-os no repositorio de telemetria ou conecte-os aos checks de lint do CI.
- O template de comunicacao de downgrade para mensagens de SDK/portal reside em `docs/source/soranet/templates/downgrade_communication_template.md`.
- Weekly status digests devem usar `docs/source/status/soranet_testnet_weekly_digest.md` como forma canonica.

Pull requests devem atualizar esta pagina junto com qualquer mudanca de artefatos ou telemetria para que o rollout plan se mantenha canonico.
