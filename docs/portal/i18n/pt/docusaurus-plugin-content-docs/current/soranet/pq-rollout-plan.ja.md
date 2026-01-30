---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/soranet/pq-rollout-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09b0384ef94eb6c36d8344005e797adc656ac9e216a92368ec3b508861a1d254
source_last_modified: "2025-11-14T04:43:22.429173+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Fonte canonica
Esta pagina espelha `docs/source/soranet/pq_rollout_plan.md`. Mantenha ambas as copias sincronizadas.
:::

SNNet-16G conclui o rollout pos-quantico do transporte SoraNet. Os knobs `rollout_phase` permitem que os operadores coordenem uma promocao deterministica do requisito atual de guard Stage A para a cobertura majoritaria Stage B e a postura PQ estrita Stage C sem editar JSON/TOML bruto para cada surface.

Este playbook cobre:

- Definicoes de fase e os novos knobs de configuracao (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) wired no codebase (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeamento de flags SDK e CLI para que cada client acompanhe o rollout.
- Expectativas de scheduling de canary relay/client e os dashboards de governance que gateiam a promocao (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks de rollback e referencias ao fire-drill runbook ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Mapa de fases

| `rollout_phase` | Estagio de anonimato efetivo | Efeito padrao | Uso tipico |
|-----------------|---------------------------|----------------|---------------|
| `canary`        | `anon-guard-pq` (Stage A) | Exigir ao menos um guard PQ por circuit enquanto a frota aquece. | Baseline e semanas iniciais de canary. |
| `ramp`          | `anon-majority-pq` (Stage B) | Viesar a selecao para relays PQ com >= dois tercos de coverage; relays classicos ficam como fallback. | Canary por regiao de relays; toggles de preview em SDK. |
| `default`       | `anon-strict-pq` (Stage C) | Forcar circuits apenas PQ e endurecer alarms de downgrade. | Promocao final apos telemetria e sign-off de governance. |

Se uma surface tambem definir `anonymity_policy` explicita, ela sobrescreve a fase para aquele componente. Omitir o stage explicito agora faz defer ao valor de `rollout_phase` para que operadores possam virar a fase uma vez por ambiente e deixar os clients herdarem.

## Referencia de configuracao

### Orchestrator (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

O loader do orchestrator resolve o stage de fallback em runtime (`crates/sorafs_orchestrator/src/lib.rs:2229`) e o expone via `sorafs_orchestrator_policy_events_total` e `sorafs_orchestrator_pq_ratio_*`. Veja `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` para snippets prontos para aplicar.

### Rust client / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` agora registra a fase parseada (`crates/iroha/src/client.rs:2315`) para que helper commands (por exemplo `iroha_cli app sorafs fetch`) reportem a fase atual junto com a politica de anonimato padrao.

## Automacao

Dois helpers `cargo xtask` automatizam a geracao do schedule e a captura de artefatos.

1. **Gerar o schedule regional**

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

   Durations aceitam sufixos `s`, `m`, `h` ou `d`. O comando emite `artifacts/soranet_pq_rollout_plan.json` e um resumo Markdown (`artifacts/soranet_pq_rollout_plan.md`) que pode ser enviado com a solicitacao de change.

2. **Capturar artefatos do drill com assinaturas**

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

   O comando copia os arquivos fornecidos para `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcula digests BLAKE3 para cada artefato e escreve `rollout_capture.json` contendo metadata e uma assinatura Ed25519 sobre o payload. Use a mesma private key que assina as minutes do fire-drill para que a governance valide a captura rapidamente.

## Matriz de flags SDK & CLI

| Surface | Canary (Stage A) | Ramp (Stage B) | Default (Stage C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` fetch | `--anonymity-policy stage-a` ou confiar na fase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator config JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust client config (`iroha.toml`) | `rollout_phase = "canary"` (default) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` signed commands | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, optional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, optional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, optional `.ANON_STRICT_PQ` |
| JavaScript orchestrator helpers | `rolloutPhase: "canary"` ou `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Todos os toggles de SDK mapeiam para o mesmo stage parser usado pelo orchestrator (`crates/sorafs_orchestrator/src/lib.rs:365`), entao deployments multi-language ficam em lock-step com a fase configurada.

## Checklist de scheduling canary

1. **Preflight (T minus 2 weeks)**

- Confirmar que o brownout rate de Stage A esta <1% nas duas ultimas semanas e que a cobertura PQ e >=70% por regiao (`sorafs_orchestrator_pq_candidate_ratio`).
   - Agendar o slot de governance review que aprova a janela de canary.
   - Atualizar `sorafs.gateway.rollout_phase = "ramp"` em staging (editar o orchestrator JSON e redeploy) e fazer dry-run do pipeline de promocao.

2. **Relay canary (T day)**

   - Promover uma regiao por vez configurando `rollout_phase = "ramp"` no orchestrator e nos manifests de relay participantes.
   - Monitorar "Policy Events per Outcome" e "Brownout Rate" no dashboard PQ Ratchet (agora com panel de rollout) por duas vezes o TTL do guard cache.
   - Fazer snapshots de `sorafs_cli guard-directory fetch` antes e depois para armazenamento de auditoria.

3. **Client/SDK canary (T plus 1 week)**

   - Trocar para `rollout_phase = "ramp"` em configs de client ou passar overrides `stage-b` para cohorts de SDK designados.
   - Capturar diffs de telemetria (`sorafs_orchestrator_policy_events_total` agrupado por `client_id` e `region`) e anexar ao rollout incident log.

4. **Default promotion (T plus 3 weeks)**

   - Depois do sign-off de governance, alterar orchestrator e client configs para `rollout_phase = "default"` e rotacionar o readiness checklist assinado para os release artefacts.

## Governance & evidence checklist

| Phase change | Promotion gate | Evidence bundle | Dashboards & alerts |
|--------------|----------------|-----------------|---------------------|
| Canary -> Ramp *(Stage B preview)* | Stage-A brownout rate <1% nos ultimos 14 dias, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 por regiao promovida, Argon2 ticket verify p95 < 50 ms, e slot de governance para a promocao reservado. | Par JSON/Markdown de `cargo xtask soranet-rollout-plan`, snapshots pareados de `sorafs_cli guard-directory fetch` (antes/depois), bundle assinado `cargo xtask soranet-rollout-capture --label canary`, e minutes de canary referenciando [PQ ratchet runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (Policy Events + Brownout Rate), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 downgrade ratio), referencias de telemetria em `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Ramp -> Default *(Stage C enforcement)* | Burn-in de telemetria SN16 de 30 dias concluido, `sn16_handshake_downgrade_total` flat no baseline, `sorafs_orchestrator_brownouts_total` zero durante o canary de client, e o rehearsal do proxy toggle logado. | Transcript `sorafs_cli proxy set-mode --mode gateway|direct`, output de `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, log de `sorafs_cli guard-directory verify`, e bundle assinado `cargo xtask soranet-rollout-capture --label default`. | O mesmo board PQ Ratchet e os panels de downgrade SN16 documentados em `docs/source/sorafs_orchestrator_rollout.md` e `dashboards/grafana/soranet_privacy_metrics.json`. |
| Emergency demotion / rollback readiness | Acionado quando counters de downgrade sobem, a verificacao do guard-directory falha, ou o buffer `/policy/proxy-toggle` registra downgrade events sustentados. | Checklist de `docs/source/ops/soranet_transport_rollback.md`, logs de `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidentes e templates de notificacao. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, e ambos alert packs (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Armazene cada artefato em `artifacts/soranet_pq_rollout/<timestamp>_<label>/` com o `rollout_capture.json` gerado para que os governance packets contenham o scoreboard, promtool traces e digests.
- Anexe digests SHA256 das evidencias carregadas (minutes PDF, capture bundle, guard snapshots) as minutes de promocao para que as aprovacoes Parliament possam ser reproduzidas sem acesso ao staging cluster.
- Referencie o plano de telemetria no ticket de promocao para provar que `docs/source/soranet/snnet16_telemetry_plan.md` segue sendo a fonte canonica para vocabularios de downgrade e alert thresholds.

## Atualizacoes de dashboard e telemetria

`dashboards/grafana/soranet_pq_ratchet.json` agora inclui um panel de anotacoes "Rollout Plan" que linka para este playbook e mostra a fase atual para que as reviews de governance confirmem qual stage esta ativo. Mantenha a descricao do panel sincronizada com mudancas futuras nos config knobs.

Para alerting, garanta que as regras existentes usem o label `stage` para que as fases canary e default disparem policy thresholds separados (`dashboards/alerts/soranet_handshake_rules.yml`).

## Rollback hooks

### Default -> Ramp (Stage C -> Stage B)

1. Faca demotion do orchestrator com `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (e espelhe a mesma fase nos SDK configs) para que Stage B retorne em toda a frota.
2. Force clients ao safe transport profile via `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, capturando o transcript para que o workflow `/policy/proxy-toggle` continue auditavel.
3. Rode `cargo xtask soranet-rollout-capture --label rollback-default` para arquivar guard-directory diffs, promtool output e dashboard screenshots em `artifacts/soranet_pq_rollout/`.

### Ramp -> Canary (Stage B -> Stage A)

1. Importe o guard-directory snapshot capturado antes da promocao com `sorafs_cli guard-directory import --guard-directory guards.json` e rode `sorafs_cli guard-directory verify` novamente para que o demotion packet inclua hashes.
2. Ajuste `rollout_phase = "canary"` (ou override com `anonymity_policy stage-a`) em orchestrator e client configs, depois rode novamente o PQ ratchet drill do [PQ ratchet runbook](./pq-ratchet-runbook.md) para provar o pipeline de downgrade.
3. Anexe screenshots atualizados de PQ Ratchet e telemetria SN16 mais os resultados de alertas ao incident log antes de notificar governance.

### Guardrail reminders

- Referencie `docs/source/ops/soranet_transport_rollback.md` sempre que ocorrer demotion e registre qualquer mitigacao temporaria como item `TODO:` no rollout tracker para acompanhamento.
- Mantenha `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` sob cobertura de `promtool test rules` antes e depois de um rollback para que o alert drift fique documentado junto ao capture bundle.
