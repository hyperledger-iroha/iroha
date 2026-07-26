---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-rollout-plan
título: Plano de despliegue poscuantico SNNet-16G
sidebar_label: Plano de distribuição PQ
description: Guia operacional para promover o handshake híbrido X25519+ML-KEM de SoraNet desde canário até padrão em relés, clientes e SDKs.
---

:::nota Fonte canônica
Esta página reflete `docs/source/soranet/pq_rollout_plan.md`. Mantenha ambas as cópias sincronizadas.
:::

SNNet-16G completa o despliegue poscuantico para o transporte de SoraNet. Os controles `rollout_phase` permitem que os operadores coordenem uma promoção determinada a partir do requisito atual de proteção do Estágio A até a cobertura maior do Estágio B e a postura PQ restrita do Estágio C sem editar JSON/TOML crudo para cada superfície.

Este playbook cubre:

- Definições de fases e os novos botões de configuração (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) conectados na base de código (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapa de flags de SDK e CLI para que cada cliente possa seguir o rollout.
- Expectativas de agendamento de retransmissão/cliente canário, mas os painéis de governança que geram a promoção (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks de rollback e referências ao runbook de fire-drill ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Mapa de fases

| `rollout_phase` | Etapa de anonimato efetivamente | Efeito por defeito | Uso típico |
|-----------------|--------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Estágio A) | Exija pelo menos um guarda PQ por circuito enquanto a flota estiver quente. | Baseline e primeiras semanas de canário. |
| `ramp` | `anon-majority-pq` (Estágio B) | Selecione a seleção para relés PQ para >= dos terços de cobertura; relés clássicos permanecem como fallback. | Canárias por região de revezamentos; alterna a visualização no SDK. |
| `default` | `anon-strict-pq` (Estágio C) | Aplica circuitos solo PQ e suporta alarmes de downgrade. | Promoção final uma vez completada a telemetria e a assinatura da governança. |

Se uma superfície também definir um `anonymity_policy` explicitamente, esta substituirá a fase para esse componente. Omitir a etapa explícita agora difere do valor de `rollout_phase` para que os operadores possam mudar a fase uma única vez por entorno e deixar que os clientes la aqui.

## Referência de configuração

### Orquestrador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

O carregador do orquestrador resolve a etapa de fallback em tempo de execução (`crates/sorafs_orchestrator/src/lib.rs:2229`) e expõe via `sorafs_orchestrator_policy_events_total` e `sorafs_orchestrator_pq_ratio_*`. Ver `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` para listas de trechos para aplicação.

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` agora registre a fase analisada (`crates/iroha/src/client.rs:2315`) para que os ajudantes (por exemplo, `iroha_cli app sorafs fetch`) possam relatar a fase atual junto com a política de anonimato por defeito.

## Automatização

Dos ajudantes `cargo xtask` automatizam a geração da programação e a captura de artefatos.

1. **Gerar a programação regional**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```As durações aceitam os sufixos `s`, `m`, `h` ou `d`. O comando emite `artifacts/soranet_pq_rollout_plan.json` e um currículo em Markdown (`artifacts/soranet_pq_rollout_plan.md`) que pode ser enviado com a solicitação de mudança.

2. **Capturar artefatos de perfuração com firmas**

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

   O comando copia os arquivos fornecidos em `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcula os resumos BLAKE3 para cada artefato e escreve `rollout_capture.json` com metadados, mas uma firma Ed25519 sobre a carga útil. Use a mesma chave privada que firma os minutos do exercício de simulação de incêndio para que a governança valide a captura rapidamente.

## Matriz de flags de SDK e CLI

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

Todos os alternadores do SDK são mapeados para o mesmo analisador de etapas usado pelo orquestrador (`crates/sorafs_orchestrator/src/lib.rs:365`), pois os aplicativos multilíngues são mantidos em lock-step com a fase definida.

## Checklist de agendamento de canário

1. **Pré-voo (T menos 2 semanas)**

- Confirmar que a taxa de brownout do Estágio A mar =70% por região (`sorafs_orchestrator_pq_candidate_ratio`).
   - Programar o slot de governança review que verifica a janela canary.
   - Atualizar `sorafs.gateway.rollout_phase = "ramp"` no teste (editar o JSON do orquestrador e reimplantar) e executar a simulação do pipeline de promoção.

2. **Retransmissão canário (dia T)**

   - Promova uma região configurando `rollout_phase = "ramp"` no orquestrador e nos manifestos de retransmissão dos participantes.
   - Monitore "Eventos de Política por Resultado" e "Taxa de Brownout" no painel PQ Ratchet (que agora inclui o painel de implementação) durante o dobro do TTL de cache de proteção.
   - Cortar snapshots de `sorafs_cli guard-directory fetch` antes e depois da execução para armazenamento de auditórios.

3. **Cliente/SDK canário (T mais 1 semana)**

   - Alterar `rollout_phase = "ramp"` nas configurações do cliente ou substituir `stage-b` para os coortes do SDK designados.
   - Capturar diferenças de telemetria (`sorafs_orchestrator_policy_events_total` agrupadas por `client_id` e `region`) e adicioná-las ao registro de incidentes de implementação.

4. **Promoção padrão (T mais 3 semanas)**- Uma vez que a governança seja firme, altere tanto o orquestrador como as configurações do cliente para `rollout_phase = "default"` e gire a lista de verificação de prontidão firmada para os artefatos de lançamento.

## Checklist de governança e evidências

| Mudança de fase | Portão de promoção | Pacote de evidências | Painéis e alertas |
|--------------|----------------|-----------------|---------------------|
| Canário -> Rampa *(prévia do Estágio B)* | Tasa de brownout Stage A = 0,7 por região promovida, Argon2 ticket verify p95  Padrão *(aplicação do Estágio C)* | Burn-in de telemetria SN16 de 30 dias cumplido, `sn16_handshake_downgrade_total` plano em linha de base, `sorafs_orchestrator_brownouts_total` em zero durante o canário do cliente, e o ensaio do proxy toggle registrado. | Transcrição de `sorafs_cli proxy set-mode --mode gateway|direct`, saída de `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, log de `sorafs_cli guard-directory verify`, e pacote firmado `cargo xtask soranet-rollout-capture --label default`. | Mismo tablero PQ Ratchet mas os painéis de downgrade SN16 documentados em `docs/source/sorafs_orchestrator_rollout.md` e `dashboards/grafana/soranet_privacy_metrics.json`. |
| Preparação para rebaixamento/reversão de emergência | Se você ativar os contadores de downgrade suben, a verificação do diretório de proteção ou o buffer `/policy/proxy-toggle` registrará eventos de downgrade sustentados. | Checklist de `docs/source/ops/soranet_transport_rollback.md`, logs de `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidentes e modelos de notificação. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` e ambos os pacotes de alertas (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Guarde cada artefato abaixo `artifacts/soranet_pq_rollout/<timestamp>_<label>/` com o `rollout_capture.json` gerado para que os pacotes de governança contenham o placar, trazas de promtool e resumos.
- Adjunta digests SHA256 de evidência elevada (minutos PDF, pacote de captura, guarda instantâneos) aos minutos de promoção para que as aprovações do Parlamento possam ser reproduzidas sem acesso ao cluster de encenação.
- Referência ao plano de telemetria no ticket de promoção para provar que `docs/source/soranet/snnet16_telemetry_plan.md` segue a fonte canônica de vocabulário de downgrade e guarda-chuvas de alerta.

## Atualizações de dashboards e telemetria

`dashboards/grafana/soranet_pq_ratchet.json` agora inclui um painel de anotações "Plano de implementação" que segue este manual e mostra a fase atual para que as revisões de governança confirmem que a etapa está ativa. Mantenha a descrição do painel sincronizada com mudanças futuras nos botões de configuração.

Para alertar, certifique-se de que as regras existentes usam a etiqueta `stage` para que as fases canário e padrão disparem limites de política separados (`dashboards/alerts/soranet_handshake_rules.yml`).

## Ganchos de reversão

### Padrão -> Rampa (Estágio C -> Estágio B)1. Baixe o orquestrador com `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (e reflita a mesma fase nas configurações do SDK) para que o Estágio B volte a toda a flota.
2. Execute os clientes no perfil de transporte seguro via `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, capturando a transcrição para que o fluxo de trabalho de remediação `/policy/proxy-toggle` seja auditável.
3. Execute `cargo xtask soranet-rollout-capture --label rollback-default` para arquivar diffs do diretório de proteção, saída do promtool e capturas de tela dos painéis abaixo do `artifacts/soranet_pq_rollout/`.

### Rampa -> Canário (Estágio B -> Estágio A)

1. Importe o instantâneo do diretório de proteção capturado antes da promoção com `sorafs_cli guard-directory import --guard-directory guards.json` e execute `sorafs_cli guard-directory verify` para que o pacote de demonstração inclua hashes.
2. Ajuste `rollout_phase = "canary"` (ou substitua `anonymity_policy stage-a`) nas configurações do orquestrador e do cliente, e depois repita a broca de catraca PQ do [PQ ratchet runbook](./pq-ratchet-runbook.md) para testar o downgrade do pipeline.
3. Capturas de tela adicionais atualizadas de PQ Ratchet e telemetria SN16 mas os resultados de alertas ao registro de incidentes antes de notificar a governança.

### Registros de guarda-corpo

- Referência `docs/source/ops/soranet_transport_rollback.md` sempre que ocorrer uma desmontagem e registrar qualquer mitigação temporal como um item `TODO:` no rollout tracker para trabalho posterior.
- Mantenha `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` sob a cobertura de `promtool test rules` antes e depois de uma reversão para que o desvio de alertas seja documentado junto com o pacote de captura.