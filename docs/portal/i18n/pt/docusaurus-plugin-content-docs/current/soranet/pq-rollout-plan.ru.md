---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-rollout-plan
título: Плейбук постквантового lançamento SNNet-16G
sidebar_label: PQ de lançamento do plano
description: Guia de operação para produzir o handshake X25519+ML-KEM SoraNet do canary por padrão para relés, clientes e SDKs.
---

:::nota História Canônica
:::

SNNet-16G suporta implementação pós-quântica para transporte SoraNet. Knobs `rollout_phase` позволяют операторам координировать детерминированный promoção от текущего Requerimento de guarda do Estágio A para cobertura majoritária do Estágio B e Postura PQ estrita do Estágio C без Redirecione seu JSON/TOML para a configuração correta.

Este manual contém:

- Fase aprimorada e novos botões de configuração (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) na base de código (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeamento de sinalizadores SDK e CLI, este cliente pode implementar o lançamento.
- Organize o agendamento canário para painéis de relé/cliente e governança, como promoção de portão (`dashboards/grafana/soranet_pq_ratchet.json`).
- Ganchos de reversão e usados ​​no runbook de simulação de incêndio ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Carta de arquivo

| `rollout_phase` | Estágio de anonimato eficaz | Efeito sobre o uso | Tipos de utilização |
|-----------------|--------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Estágio A) | Требовать как минимум один PQ guard на circuito, пока флот прогревается. | Linha de base e ранние недели canário. |
| `ramp` | `anon-majority-pq` (Estágio B) | Смещать выбор в сторону Relés PQ para >= двух третей cobertura; relés clássicos остаются fallback. | Canários de retransmissão regionais; A visualização do SDK alterna. |
| `default` | `anon-strict-pq` (Estágio C) | Use circuitos somente PQ e use alarmes de downgrade. | A promoção final permite a telemetria e a governança de aprovação. |

Você também pode usar o `anonymity_policy`, na fase de ajuste para este componente. Отсутствие явного stage теперь defer-ится к значению `rollout_phase`, чтобы операторы могли переключить fase один раз на среду и дать clientes унаследовать его.

## Referência de configuração

### Orquestrador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

O orquestrador do carregador configurou o estágio de fallback no momento certo (`crates/sorafs_orchestrator/src/lib.rs:2229`) e o executou `sorafs_orchestrator_policy_events_total` e `sorafs_orchestrator_pq_ratio_*`. Sim. `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` para trechos obtidos.

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` é uma fase de expansão de fase (`crates/iroha/src/client.rs:2315`), comandos auxiliares (por exemplo `iroha_cli app sorafs fetch`) grandes сообщать текущую fase вместе с política de anonimato padrão.

## Automação

O ajudante `cargo xtask` автоматизируют gera clareza e captura de artefatos.

1. **Programação regional de agendamento**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   As durações são suficientes `s`, `m`, `h` ou `d`. O comando usa `artifacts/soranet_pq_rollout_plan.json` e Resumo de Markdown (`artifacts/soranet_pq_rollout_plan.md`), que pode ser usado para solicitação de alteração.

2. **Capturar artefatos de perfuração com suporte**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```Команда копирует указанные файлы в `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, вычисляет BLAKE3 digests para o artefato каждого e o item `rollout_capture.json` com metadados и Ed25519 fornece carga útil superior. Use sua chave privada, que fornece minutos de simulação de incêndio, que governam a captura.

## Матрица флагов SDK e CLI

| Superfície | Canário (Fase A) | Rampa (Etapa B) | Padrão (Fase C) |
|---------|------------------|----------------|------------------|
| `sorafs_cli` buscar | `--anonymity-policy stage-a` ou operação de fase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuração do orquestrador JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuração do cliente Rust (`iroha.toml`) | `rollout_phase = "canary"` (padrão) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Comandos assinados `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, opcional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, opcional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, opcional `.ANON_STRICT_PQ` |
| Ajudantes do orquestrador JavaScript | `rolloutPhase: "canary"` ou `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Este SDK alterna o mapa para o analisador de estágio, para o orquestrador (`crates/sorafs_orchestrator/src/lib.rs:365`), para implantações multilíngues na fase de bloqueio de etapa.

## Lista de verificação de agendamento canário

1. **Pré-voo (T menos 2 semanas)**

- Убедитесь, что Estágio A taxa de brownout =70% na região (`sorafs_orchestrator_pq_candidate_ratio`).
   - Запланируйте slot de revisão de governança, который утверждает окно canário.
   - Ative `sorafs.gateway.rollout_phase = "ramp"` no teste (use o orquestrador JSON e reimplante) e use o pipeline de promoção de simulação.

2. **Retransmissão canário (dia T)**

   - Produza em uma região específica, coloque `rollout_phase = "ramp"` no orquestrador e manifeste os relés de uso.
   - Selecione "Eventos de Política por Resultado" e "Taxa de Queda de Energia" no painel do PQ Ratchet (no painel de implementação) com a tecnologia TTL Guard Cache atualizada.
   - Снимайте snapshots `sorafs_cli guard-directory fetch` para fazer e usar para armazenamento de auditoria.

3. **Cliente/SDK canário (T mais 1 semana)**

   - Selecione `rollout_phase = "ramp"` nas configurações do cliente ou substitua substituições `stage-b` para coortes SDK de vários usuários.
   - Abra diferenças de telemetria (`sorafs_orchestrator_policy_events_total`, сгруппированные по `client_id` e `region`) e use-o para log de incidentes de implementação.

4. **Promoção padrão (T mais 3 semanas)**

   - Coloque a governança de assinatura para configurar o orquestrador e as configurações do cliente em `rollout_phase = "default"` e configure a lista de verificação de prontidão para liberar artefatos.

## Lista de verificação de governança e evidências| Mudança de fase | Portão de promoção | Pacote de evidências | Painéis e alertas |
|--------------|----------------|-----------------|---------------------|
| Canário -> Rampa *(prévia do Estágio B)* | Taxa de brownout do estágio A = 0,7 para promoção regional, verificação de ticket Argon2 p95  Padrão *(aplicação do Estágio C)* | 30-дневный SN16 telemetria burn-in выполнен, `sn16_handshake_downgrade_total` плоский на linha de base, `sorafs_orchestrator_brownouts_total` ноль во во время cliente canário, e proxy alternar ensaio записан. | Transcreva `sorafs_cli proxy set-mode --mode gateway|direct`, você precisa `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, лог `sorafs_cli guard-directory verify`, подписанный pacote `cargo xtask soranet-rollout-capture --label default`. | Para esta placa de catraca PQ mais painéis de downgrade SN16, consulte `docs/source/sorafs_orchestrator_rollout.md` e `dashboards/grafana/soranet_privacy_metrics.json`. |
| Preparação para rebaixamento/reversão de emergência | Acione os contadores de downgrade, verifique a verificação do diretório de proteção ou use eventos de downgrade no buffer `/policy/proxy-toggle`. | Lista de verificação de `docs/source/ops/soranet_transport_rollback.md`, logs `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidentes e modelos de notificação. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` e outros pacotes de alerta (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Храните каждый artefato em `artifacts/soranet_pq_rollout/<timestamp>_<label>/` вместе с сгенерированным `rollout_capture.json`, pacotes de governança чтобы включали placar, traços de promtool e resumos.
- Приложите SHA256 digests загруженных доказательств (minutos PDF, pacote de captura, guarda instantâneos) к promoção minutos, чтобы Aprovações do Parlamento можно было воспроизвести без fornecido para cluster de teste.
- Selecione o plano de telemetria no bilhete de promoção, чтобы подтвердить, что `docs/source/soranet/snnet16_telemetry_plan.md` остается каноническим источником para rebaixar vocabulários e limites de alerta.

## Atualizações de painel e telemetria

`dashboards/grafana/soranet_pq_ratchet.json` contém o painel de anotação "Plano de implementação", que contém este manual e a fase de implementação da fase, as revisões de governança podem ser encontradas estágio ativo. Selecione a descrição do painel de sincronização com os botões de ajuste corretos.

Para alertar убедитесь, что существующие regras используют rótulo `stage`, чтобы canário e fases padrão триггерили отдельные limites de política (`dashboards/alerts/soranet_handshake_rules.yml`).

## Ganchos de reversão

### Padrão -> Rampa (Estágio C -> Estágio B)

1. Rebaixar o orquestrador de `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (e отзеркалить ту же phase in SDK configs), чтобы Stage B восстановился по всему флоту.
2. Coloque os clientes no perfil de transporte específico do `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, transcrição transcrita para o fluxo de trabalho de auditabilidade `/policy/proxy-toggle`.
3. Abra `cargo xtask soranet-rollout-capture --label rollback-default` para obter diferenças de diretório de guarda, saída do promtool e capturas de tela do painel para `artifacts/soranet_pq_rollout/`.

### Rampa -> Canário (Estágio B -> Estágio A)1. Importe o instantâneo do diretório de proteção, promova a promoção, selecione `sorafs_cli guard-directory import --guard-directory guards.json` e remova o `sorafs_cli guard-directory verify`, rebaixe-o pacote contém hashes.
2. Configure `rollout_phase = "canary"` (ou substitua `anonymity_policy stage-a`) nas configurações do orquestrador e do cliente, use a broca de catraca PQ em [PQ ratchet runbook](./pq-ratchet-runbook.md), чтобы доказать pipeline de downgrade.
3. Obtenha capturas de tela de telemetria PQ Ratchet e SN16, além de resultados de alerta de resultados de alerta e registro de incidentes para governança corporativa.

### Lembretes de proteção

- Selecione `docs/source/ops/soranet_transport_rollback.md` para rebaixamento e фиксируйте временные mitigação como `TODO:` no rastreador de implementação para implementação trabalho.
- Selecione `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` para ativar `promtool test rules` e executar rollback, alerta de desvio que será ativado вместе с pacote de captura.