---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-rollout-plan
título: Manual de implementação pós-quântico SNNet-16G
sidebar_label: Plano de implementação PQ
description: Guia operacional para promover o handshake híbrido X25519+ML-KEM do SoraNet de canary para padrão em relés, clientes e SDKs.
---

:::nota Fonte canônica
Esta página espelha `docs/source/soranet/pq_rollout_plan.md`. Mantenha ambas as cópias sincronizadas.
:::

SNNet-16G conclui o rollout pós-quântico do transporte SoraNet. Os botões `rollout_phase` permitem que os operadores coordenem uma promoção determinística do requisito atual de guarda Estágio A para a cobertura majoritária Estágio B e a postura PQ estrita Estágio C sem editar JSON/TOML bruto para cada superfície.

Este manual cobre:

- Definições de fase e os novos botões de configuração (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) com fio sem base de código (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeamento de flags SDK e CLI para que cada cliente acompanhe o rollout.
- Expectativas de agendamento de canary relay/client e dos dashboards de governança que foram acionados para promoção (`dashboards/grafana/soranet_pq_ratchet.json`).
- Ganchos de rollback e referências ao runbook de simulação de incêndio ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Mapa de fases

| `rollout_phase` | Estágio de anonimato efetivo | Efeito padrão | Uso típico |
|-----------------|--------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Estágio A) | Exigir ao menos um guarda PQ por circuito enquanto a frota aquece. | Linha de base e semanas iniciais de canary. |
| `ramp` | `anon-majority-pq` (Estágio B) | Viesar a seleção para relés PQ com >= dois tercos de cobertura; os relés clássicos ficam como fallback. | Canário por regiao de revezamentos; alterna a visualização no SDK. |
| `default` | `anon-strict-pq` (Estágio C) | Circuitos Forcar apenas PQ e suportar alarmes de downgrade. | Promoção final após telemetria e aprovação de governança. |

Se uma superfície também definir `anonymity_policy` explicitamente, ela sobrescreve a fase para aquele componente. Omitir o estágio explícito agora faz adiar o valor de `rollout_phase` para que os operadores possam mudar de fase uma vez por ambiente e deixar os clientes herdarem.

## Referência de configuração

### Orquestrador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

O loader do orquestrador resolve o estágio de fallback em runtime (`crates/sorafs_orchestrator/src/lib.rs:2229`) e o expõe via `sorafs_orchestrator_policy_events_total` e `sorafs_orchestrator_pq_ratio_*`. Veja `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` para trechos prontos para aplicação.

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` agora registra a fase analisada (`crates/iroha/src/client.rs:2315`) para que os comandos auxiliares (por exemplo `iroha_cli app sorafs fetch`) relatem a fase atual junto com a política de anonimato padrão.

## Automação

Dois helpers `cargo xtask` automatizam a geração do cronograma e a captura de artistas.

1. **Gerar agendamento regional**

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

   As durações aceitam os sufixos `s`, `m`, `h` ou `d`. O comando emite `artifacts/soranet_pq_rollout_plan.json` e um resumo Markdown (`artifacts/soranet_pq_rollout_plan.md`) que pode ser enviado com uma solicitação de alteração.2. **Capturar artistas do drill com assinaturas**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   O comando copia os arquivos fornecidos para `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcula os resumos BLAKE3 para cada artigo e escreve `rollout_capture.json` contendo metadados e uma assinatura Ed25519 sobre o payload. Use a mesma chave privada que assina os minutos do fire-drill para que a governança valide a captura rapidamente.

## Matriz de flags SDK e CLI

| Superfície | Canário (Fase A) | Rampa (Etapa B) | Padrão (Fase C) |
|---------|------------------|----------------|------------------|
| `sorafs_cli` buscar | `--anonymity-policy stage-a` ou confiar na fase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuração do orquestrador JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuração do cliente Rust (`iroha.toml`) | `rollout_phase = "canary"` (padrão) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Comandos assinados `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, opcional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, opcional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, opcional `.ANON_STRICT_PQ` |
| Ajudantes do orquestrador JavaScript | `rolloutPhase: "canary"` ou `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Todos os toggles do SDK mapeiam para o mesmo analisador de estágio usado pelo orquestrador (`crates/sorafs_orchestrator/src/lib.rs:365`), então implantações multilíngues ficam em lock-step com a fase definida.

## Checklist de agendamento canário

1. **Pré-voo (T menos 2 semanas)**

- Confirmar que a taxa de brownout da Etapa A está =70% por regiao (`sorafs_orchestrator_pq_candidate_ratio`).
   - Agendar o slot de governança review que aprova a janela de canary.
   - Atualizar `sorafs.gateway.rollout_phase = "ramp"` em staging (editar o orquestrador JSON e reimplementar) e fazer dry-run do pipeline de promoção.

2. **Retransmissão canário (dia T)**

   - Promova uma regiao por vez configurando `rollout_phase = "ramp"` no orquestrador e nos manifestos de retransmissão participantes.
   - Monitorar “Policy Events per Outcome” e “Brownout Rate” no dashboard PQ Ratchet (agora com painel de rollout) por duas vezes o TTL do guard cache.
   - Fazer snapshots de `sorafs_cli guard-directory fetch` antes e depois para armazenamento de auditórios.

3. **Cliente/SDK canário (T mais 1 semana)**

   - Trocar para `rollout_phase = "ramp"` em configurações de cliente ou passar substituições `stage-b` para coortes de SDK designados.
   - Capturar diferenças de telemetria (`sorafs_orchestrator_policy_events_total` agrupadas por `client_id` e `region`) e anexar ao rollout incidente log.

4. **Promoção padrão (T mais 3 semanas)**

   - Após a aprovação da governança, altere as configurações do orquestrador e do cliente para `rollout_phase = "default"` e rotacione a lista de verificação de prontidão assinada para os artefatos de liberação.

## Lista de verificação de governança e evidências| Mudança de fase | Portão de promoção | Pacote de evidências | Painéis e alertas |
|--------------|----------------|-----------------|---------------------|
| Canário -> Rampa *(prévia do Estágio B)* | Taxa de brownout Stage-A = 0.7 por regiao promovida, Argon2 ticket verify p95  Padrão *(aplicação do Estágio C)* | Burn-in de telemetria SN16 de 30 dias concluído, `sn16_handshake_downgrade_total` flat no baseline, `sorafs_orchestrator_brownouts_total` zero durante o canário de cliente, e o ensaio do proxy toggle logado. | Transcrição `sorafs_cli proxy set-mode --mode gateway|direct`, saída de `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, log de `sorafs_cli guard-directory verify`, e pacote assinado `cargo xtask soranet-rollout-capture --label default`. | A mesma placa PQ Ratchet e os painéis de downgrade SN16 documentados em `docs/source/sorafs_orchestrator_rollout.md` e `dashboards/grafana/soranet_privacy_metrics.json`. |
| Preparação para rebaixamento/reversão de emergência | Acionado quando os contadores de downgrade sobem, a verificação do guard-directory falha, ou o buffer `/policy/proxy-toggle` registra eventos de downgrade sustentados. | Checklist de `docs/source/ops/soranet_transport_rollback.md`, logs de `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidentes e modelos de notificação. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, e ambos os pacotes de alerta (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Armazene cada artista em `artifacts/soranet_pq_rollout/<timestamp>_<label>/` com o `rollout_capture.json` gerado para que os pacotes de governança contenham o scoreboard, promtool traces e digests.
- Anexo digests SHA256 das evidencias transmitidas (minutos PDF, pacote de captura, guarda instantâneos) como minutos de promoção para que as aprovações do Parlamento possam ser reproduzidas sem acesso ao staging cluster.
- Referência o plano de telemetria no ticket de promoção para provar que `docs/source/soranet/snnet16_telemetry_plan.md` segue sendo a fonte canônica para vocabulários de downgrade e limites de alerta.

## Atualizações de dashboard e telemetria

`dashboards/grafana/soranet_pq_ratchet.json` agora inclui um painel de anotações "Plano de Rollout" que está vinculado a este playbook e mostra a fase atual para que as revisões de governança confirmem qual estágio está ativo. Mantenha a descrição do painel sincronizada com mudanças futuras nos botões de configuração.

Para alertar, garanta que as regras existentes usem o rótulo `stage` para que as fases canárias e padrão disparem limites de política separados (`dashboards/alerts/soranet_handshake_rules.yml`).

## Ganchos de reversão

### Padrão -> Rampa (Estágio C -> Estágio B)1. Faca rebaixamento do orquestrador com `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (e espelhe a mesma fase nas configurações do SDK) para que o Stage B retorne em toda a frota.
2. Forçar os clientes ao perfil de transporte seguro via `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, capturando a transcrição para que o fluxo de trabalho `/policy/proxy-toggle` continue auditável.
3. Rode `cargo xtask soranet-rollout-capture --label rollback-default` para arquivar diffs do diretório de guarda, saída do promtool e capturas de tela do painel em `artifacts/soranet_pq_rollout/`.

### Rampa -> Canário (Estágio B -> Estágio A)

1. Importe o snapshot do diretório de proteção capturado antes da promoção com `sorafs_cli guard-directory import --guard-directory guards.json` e rode `sorafs_cli guard-directory verify` novamente para que o pacote de rebaixamento incluindo hashes.
2. Ajuste `rollout_phase = "canary"` (ou override com `anonymity_policy stage-a`) nas configurações do orquestrador e do cliente, depois rode novamente o PQ ratchet drill do [PQ ratchet runbook](./pq-ratchet-runbook.md) para provar o pipeline de downgrade.
3. Anexo screenshots atualizados de PQ Ratchet e telemetria SN16 mais os resultados de alertas ao log de incidentes antes de notificar governança.

### Lembretes de proteção

- Referência `docs/source/ops/soranet_transport_rollback.md` sempre que ocorrer rebaixamento e registrar qualquer mitigação temporária como item `TODO:` no rollout tracker para acompanhamento.
- Mantenha `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` sob cobertura de `promtool test rules` antes e depois de um rollback para que o alerta drift fique documentado junto ao pacote de captura.