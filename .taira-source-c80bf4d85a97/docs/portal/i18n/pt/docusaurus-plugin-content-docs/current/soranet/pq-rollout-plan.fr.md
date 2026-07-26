---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-rollout-plan
título: Manual de implantação pós-quantique SNNet-16G
sidebar_label: Plano de implantação PQ
descrição: Guia operacional para promover o handshake híbrido X25519+ML-KEM de SoraNet de canary como padrão em relés, clientes e SDKs.
---

:::nota Fonte canônica
:::

SNNet-16G termina a implantação pós-quantica para o transporte SoraNet. Os botões `rollout_phase` permitem aos operadores de coordenação uma promoção determinada pelo requisito de proteção do Estágio A em relação à cobertura majoritária do Estágio B e à postura PQ estrita do Estágio C sem o modificador JSON/TOML bruto para cada superfície.

Este manual de instruções:

- Definições de fase e novos botões de configuração (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) cabos na base de código (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeamento de sinalizadores SDK e CLI para que cada cliente possa seguir o lançamento.
- Atentes de agendamento de relé/cliente canário e painéis de governança que permitem a promoção (`dashboards/grafana/soranet_pq_ratchet.json`).
- Ganchos de reversão e referências ao runbook de simulação de incêndio ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Carta das fases

| `rollout_phase` | Etapa de anonimato eficaz | Efeito por padrão | Tipo de uso |
|-----------------|--------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Estágio A) | Exija pelo menos uma proteção PQ no circuito enquanto a flotte for recarregada. | Linha de base e estreias das semanas canárias. |
| `ramp` | `anon-majority-pq` (Estágio B) | Favorecer a seleção dos relés PQ para >= dois níveis de cobertura; os relés clássicos permanecem em reserva. | Relés canários por região; alterna o SDK de visualização. |
| `default` | `anon-strict-pq` (Estágio C) | Imponha circuitos exclusivos PQ e gere alarmes de downgrade. | A promoção é finalizada antes da telemetria e a governança de assinatura é concluída. |

Se uma superfície definida também for `anonymity_policy` explícita, ela substituirá a fase para este composto. Deixe a etapa explícita adiar a manutenção do valor `rollout_phase` para que os operadores possam baixá-los uma vez pelo ambiente e deixar os clientes herdados.

## Referência de configuração

### Orquestrador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

O carregador do orquestrador retorna a fita substituta em tempo de execução (`crates/sorafs_orchestrator/src/lib.rs:2229`) e a exposição via `sorafs_orchestrator_policy_events_total` e `sorafs_orchestrator_pq_ratio_*`. Veja `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` para os fragmentos como um aplicador.

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` registra a manutenção da fase analisada (`crates/iroha/src/client.rs:2315`) para que os comandos helper (por exemplo `iroha_cli app sorafs fetch`) possam reportar a fase atual nas linhas da política anônima por padrão.

## Automatização

Dois ajudantes `cargo xtask` automatizam a geração do cronograma e a captura de artefatos.

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
   ```As durações aceitam os sufixos `s`, `m`, `h` ou `d`. O comando emet `artifacts/soranet_pq_rollout_plan.json` e um resume Markdown (`artifacts/soranet_pq_rollout_plan.md`) para se juntar à demanda de mudança.

2. **Capture os artefatos da perfuração com assinaturas**

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

   Ao comando, copie os arquivos fornecidos em `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcule os resumos BLAKE3 para cada artefato e escreva `rollout_capture.json` com metadados mais uma assinatura Ed25519 na carga útil. Utilize a chave privada meme que assina os minutos do treinamento de incêndio para que a governança possa validar rapidamente a captura.

## Matriz de sinalizadores SDK e CLI

| Superfície | Canário (Fase A) | Rampa (Etapa B) | Padrão (Fase C) |
|---------|------------------|----------------|------------------|
| `sorafs_cli` buscar | `--anonymity-policy stage-a` ou se repousa na fase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuração do orquestrador JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuração do cliente Rust (`iroha.toml`) | `rollout_phase = "canary"` (padrão) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Comandos assinados `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, opcional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, opcional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, opcional `.ANON_STRICT_PQ` |
| Ajudantes do orquestrador JavaScript | `rolloutPhase: "canary"` ou `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Todos os toggles SDK são mapeados no analisador de memes de palco usando o orquestrador (`crates/sorafs_orchestrator/src/lib.rs:365`), de modo que as implantações de vários idiomas permanecem em lock-step com a configuração de fase.

## Checklist de agendamento canário

1. **Pré-voo (T menos 2 semanas)**

- Confirmar que a taxa de brownout Estágio A é =70% por região (`sorafs_orchestrator_pq_candidate_ratio`).
   - Planeje o slot de revisão de governança que aprova a janela canária.
   - Instale `sorafs.gateway.rollout_phase = "ramp"` no staging (editor do orquestrador JSON e redeployer) e execute o pipeline de promoção.

2. **Retransmissão canário (dia T)**

   - Promova uma região à la fois en definisant `rollout_phase = "ramp"` no orquestrador e nos manifestos dos participantes dos revezamentos.
   - Monitore "Eventos de Política por Resultado" e "Taxa de Queda de Energia" no painel PQ Ratchet (que inclui a manutenção do lançamento do painel) enquanto o TTL do cache de proteção é mantido.
   - Capturador de instantâneos `sorafs_cli guard-directory fetch` antes e depois para armazenamento de auditoria.

3. **Cliente/SDK canário (T mais 1 semana)**

   - Bascule `rollout_phase = "ramp"` nas configurações do cliente ou passe as substituições `stage-b` para os coortes SDK designados.
   - Capture diferenças de telemetria (grupo `sorafs_orchestrator_policy_events_total` par `client_id` e `region`) e junte-as no registro de incidentes de implementação.4. **Promoção padrão (T mais 3 semanas)**

   - Uma vez que a governança foi validada, o orquestrador básico e as configurações do cliente versão `rollout_phase = "default"` e fizeram a lista de verificação de prontidão assinada nos artefatos de lançamento.

## Lista de verificação de governança e evidências

| Mudança de fase | Portão de promoção | Pacote de evidências | Painéis e alertas |
|--------------|----------------|-----------------|---------------------|
| Canário -> Rampa *(prévia do Estágio B)* | Taxa de brownout Estágio A = 0,7 por região promue, verificação de ticket Argon2 p95  Padrão *(aplicação do Estágio C)* | Burn-in telemetria SN16 de 30 dias atteint, `sn16_handshake_downgrade_total` plat au baseline, `sorafs_orchestrator_brownouts_total` um zero durante o cliente canary, et ensaio du proxy toggle logge. | Transcrição `sorafs_cli proxy set-mode --mode gateway|direct`, sortie `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, log `sorafs_cli guard-directory verify`, e pacote sinal `cargo xtask soranet-rollout-capture --label default`. | Meme tableau PQ Ratchet mais os painéis SN16 downgrade documentados em `docs/source/sorafs_orchestrator_rollout.md` e `dashboards/grafana/soranet_privacy_metrics.json`. |
| Preparação para rebaixamento/reversão de emergência | Diminua quando os computadores fazem downgrade, a verificação guard-directory echoue, ou o buffer `/policy/proxy-toggle` registra os eventos de downgrade soutenus. | Checklist `docs/source/ops/soranet_transport_rollback.md`, logs `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidente e modelos de notificação. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` e os dois pacotes de alertas (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Stockez cada artefato sob `artifacts/soranet_pq_rollout/<timestamp>_<label>/` com `rollout_capture.json` gerado para que a governança de pacotes contenha o placar, os rastreamentos da ferramenta de promoção e os resumos.
- Anexe os resumos SHA256 dos pré-pagos cobrados (minutos PDF, pacote de captura, instantâneos de proteção) aux minutos de promoção para que as aprovações do Parlamento possam ser desfrutadas sem acesso ao cluster staging.
- Consulte o plano de telemetria no ticket de promoção para provar que `docs/source/soranet/snnet16_telemetry_plan.md` resta a fonte canônica de vocabulários de downgrade e seus alertas.

## Mise a jour painéis e telemetria

`dashboards/grafana/soranet_pq_ratchet.json` inclui a manutenção de um painel de anotação "Plano de implantação" que é enviado a este manual e expõe a fase atual após as revistas de governança confirmarem que o estágio está ativo. Gardez a descrição do painel sincronizado com as futuras evoluções dos botões de configuração.

Para o alerta, certifique-se de que as regras existentes utilizem o rótulo `stage` para que as fases canárias e o padrão diminuam os limites de política separados (`dashboards/alerts/soranet_handshake_rules.yml`).

## Ganchos de reversão

### Padrão -> Rampa (Estágio C -> Estágio B)1. Retroceda o orquestrador com `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (e faça a visualização da fase meme nas configurações do SDK) para que o Estágio B seja exibido em toda a flotte.
2. Force os clientes a usar o perfil de transporte em `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, capturando a transcrição para que o fluxo de trabalho de remediação `/policy/proxy-toggle` seja auditável.
3. Execute `cargo xtask soranet-rollout-capture --label rollback-default` para arquivar os diffs guard-directory, a saída promtool e as capturas de tela dos painéis sob `artifacts/soranet_pq_rollout/`.

### Rampa -> Canário (Estágio B -> Estágio A)

1. Importe a captura de snapshot guard-directory antes da promoção com `sorafs_cli guard-directory import --guard-directory guards.json` e relance `sorafs_cli guard-directory verify` para que o pacote de rebaixamento inclua os hashes.
2. Defina `rollout_phase = "canary"` (ou substitua por `anonymity_policy stage-a`) no orquestrador e nas configurações do cliente e, em seguida, execute a broca de catraca PQ do [PQ ratchet runbook](./pq-ratchet-runbook.md) para verificar o downgrade do pipeline.
3. Anexe as capturas de tela atualizadas da telemetria PQ Ratchet e SN16, bem como os resultados de alertas no registro de incidentes antes da governança de notificação.

### Guarda-corpo de rapel

- Consulte `docs/source/ops/soranet_transport_rollback.md` para cada rebaixamento e registre toda a mitigação temporária como o item `TODO:` no rastreador de implementação para seguir.
- Gardez `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` sob cobertura `promtool test rules` antes e após uma reversão para que todos os alertas sejam derivados, documentados com o pacote de captura.