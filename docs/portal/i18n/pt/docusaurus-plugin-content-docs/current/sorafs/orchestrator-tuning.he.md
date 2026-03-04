---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/orchestrator-tuning.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2519e56dc2e20e307bc0eeb6ff3d010fbf0ac623982f5840584f43d9eba38bd0
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: orchestrator-tuning
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canônica
Espelha `docs/source/sorafs/developer/orchestrator_tuning.md`. Mantenha as duas cópias alinhadas até que a documentação alternativa seja aposentada.
:::

# Guia de rollout e ajuste do orquestrador

Este guia se baseia na [referência de configuração](orchestrator-config.md) e no
[runbook de rollout multi-origem](multi-source-rollout.md). Ele explica
como ajustar o orquestrador para cada fase de rollout, como interpretar os
artefatos do scoreboard e quais sinais de telemetria devem estar prontos antes
de ampliar o tráfego. Aplique as recomendações de forma consistente na CLI, nos
SDKs e na automação para que cada nó siga a mesma política de fetch determinística.

## 1. Conjuntos de parâmetros base

Parta de um template de configuração compartilhado e ajuste um pequeno conjunto
de knobs à medida que o rollout avança. A tabela abaixo captura os valores
recomendados para as fases mais comuns; os valores não listados voltam aos
padrões de `OrchestratorConfig::default()` e `FetchOptions::default()`.

| Fase | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Notas |
|------|-----------------|-------------------------------|------------------------------------|-----------------------------|------------------------------------|-------|
| **Lab / CI** | `3` | `2` | `2` | `2500` | `300` | Um limite de latência e uma janela de graça estreitos expõem telemetria ruidosa rapidamente. Mantenha retries baixos para revelar manifestos inválidos mais cedo. |
| **Staging** | `4` | `3` | `3` | `4000` | `600` | Espelha os padrões de produção deixando folga para peers exploratórios. |
| **Canary** | `6` | `3` | `3` | `5000` | `900` | Igual aos padrões; defina `telemetry_region` para que os dashboards possam separar o tráfego canário. |
| **Disponibilidade geral** | `None` (usar todos os elegíveis) | `4` | `4` | `5000` | `900` | Aumente os limiares de retry e falha para absorver falhas transitórias enquanto as auditorias continuam reforçando o determinismo. |

- `scoreboard.weight_scale` permanece no padrão `10_000` a menos que um sistema downstream exija outra resolução inteira. Aumentar a escala não altera a ordenação dos provedores; apenas emite uma distribuição de créditos mais densa.
- Ao migrar entre fases, persista o bundle JSON e use `--scoreboard-out` para que a trilha de auditoria registre o conjunto exato de parâmetros.

## 2. Higiene do scoreboard

O scoreboard combina requisitos do manifesto, anúncios de provedores e telemetria.
Antes de avançar:

1. **Valide a frescura da telemetria.** Garanta que os snapshots referenciados por
   `--telemetry-json` foram capturados dentro da janela de graça configurada. Entradas
   mais antigas que `telemetry_grace_secs` falham com `TelemetryStale { last_updated }`.
   Trate isso como um bloqueio rígido e atualize a exportação de telemetria antes de seguir.
2. **Inspecione razões de elegibilidade.** Persista artefatos via
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Cada entrada
   traz um bloco `eligibility` com a causa exata da falha. Não sobrescreva
   desajustes de capacidade ou anúncios expirados; corrija o payload upstream.
3. **Revise mudanças de peso.** Compare o campo `normalised_weight` com o release
   anterior. Mudanças >10 % devem correlacionar com alterações deliberadas em anúncios
   ou telemetria e precisam ser registradas no log de rollout.
4. **Arquive artefatos.** Configure `scoreboard.persist_path` para que cada execução
   emita o snapshot final do scoreboard. Anexe o artefato ao registro de release
   junto ao manifesto e ao bundle de telemetria.
5. **Registre evidências de mix de provedores.** A metadata de `scoreboard.json` _e_ o
   `summary.json` correspondente devem expor `provider_count`, `gateway_provider_count`
   e o label derivado `provider_mix` para que os revisores provem se a execução foi
   `direct-only`, `gateway-only` ou `mixed`. Capturas gateway reportam `provider_count=0`
   e `provider_mix="gateway-only"`, enquanto execuções mistas exigem contagens não
   zero para ambas as fontes. `cargo xtask sorafs-adoption-check` impõe esses campos
   (e falha quando contagens/labels divergem), então execute-o sempre junto com
   `ci/check_sorafs_orchestrator_adoption.sh` ou seu script de captura para produzir
   o bundle de evidência `adoption_report.json`. Quando gateways Torii estiverem
   envolvidos, mantenha `gateway_manifest_id`/`gateway_manifest_cid` na metadata do
   scoreboard para que o gate de adoção consiga correlacionar o envelope do manifesto
   com o mix de provedores capturado.

Para definições detalhadas de campos, veja
`crates/sorafs_car/src/scoreboard.rs` e a estrutura de resumo do CLI exposta por
`sorafs_cli fetch --json-out`.

## Referência de flags da CLI e SDK

`sorafs_cli fetch` (ver `crates/sorafs_car/src/bin/sorafs_cli.rs`) e o wrapper
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) compartilham
a mesma superfície de configuração do orquestrador. Use os seguintes flags ao
capturar evidências de rollout ou ao reproduzir os fixtures canônicos:

Referência compartilhada de flags multi-origem (mantenha a ajuda da CLI e os docs
sincronizados editando apenas este arquivo):

- `--max-peers=<count>` limita quantos provedores elegíveis sobrevivem ao filtro do scoreboard. Deixe sem configurar para fazer streaming de todos os provedores elegíveis e defina `1` apenas quando estiver exercitando deliberadamente o fallback de fonte única. Espelha o knob `maxPeers` nos SDKs (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` encaminha para o limite de retries por chunk aplicado por `FetchOptions`. Use a tabela de rollout no guia de ajuste para os valores recomendados; execuções de CLI que coletam evidências devem corresponder aos padrões dos SDKs para manter a paridade.
- `--telemetry-region=<label>` rotula as séries Prometheus `sorafs_orchestrator_*` (e relés OTLP) com um label de região/ambiente para que os dashboards separem tráfego de lab, staging, canary e GA.
- `--telemetry-json=<path>` injeta o snapshot referenciado pelo scoreboard. Persista o JSON ao lado do scoreboard para que auditores possam reproduzir a execução (e para que `cargo xtask sorafs-adoption-check --require-telemetry` prove qual stream OTLP alimentou a captura).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) habilitam os hooks do observador bridge. Quando definidos, o orquestrador transmite chunks através do proxy Norito/Kaigi local para que clientes de navegador, guard caches e salas Kaigi recebam os mesmos recibos emitidos por Rust.
- `--scoreboard-out=<path>` (opcionalmente com `--scoreboard-now=<unix_secs>`) persiste o snapshot de elegibilidade para auditores. Sempre emparelhe o JSON persistido com os artefatos de telemetria e manifesto referenciados no ticket de release.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` aplicam ajustes determinísticos sobre a metadata de anúncios. Use esses flags apenas para ensaios; downgrades de produção devem passar por artefatos de governança para que cada nó aplique o mesmo bundle de política.
- `--provider-metrics-out` / `--chunk-receipts-out` retêm métricas de saúde por provedor e recibos de chunks referenciados pela checklist de rollout; anexe ambos os artefatos ao registrar a evidência de adoção.

Exemplo (usando o fixture publicado):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

Os SDKs consomem a mesma configuração via `SorafsGatewayFetchOptions` no cliente
Rust (`crates/iroha/src/client.rs`), nos bindings JS
(`javascript/iroha_js/src/sorafs.js`) e no SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Mantenha esses helpers em
sincronia com os padrões da CLI para que operadores possam copiar políticas entre
automação sem camadas de tradução sob medida.

## 3. Ajuste da política de fetch

`FetchOptions` controla retry, concorrência e verificação. Ao ajustar:

- **Retries:** Elevar `per_chunk_retry_limit` acima de `4` aumenta o tempo de
  recuperação, mas pode mascarar falhas de provedores. Prefira manter `4` como
  teto e confiar na rotação de provedores para expor os fracos.
- **Limiar de falhas:** `provider_failure_threshold` define quando um provedor é
  desabilitado pelo restante da sessão. Alinhe esse valor à política de retry:
  um limiar menor que o orçamento de retry força o orquestrador a ejetar um peer
  antes de esgotar todos os retries.
- **Concorrência:** Deixe `global_parallel_limit` sem definir (`None`) a menos que
  um ambiente específico não consiga saturar os ranges anunciados. Quando definido,
  garanta que o valor seja ≤ à soma dos orçamentos de streams dos provedores para
  evitar starvation.
- **Toggles de verificação:** `verify_lengths` e `verify_digests` devem permanecer
  habilitados em produção. Eles garantem determinismo quando há frotas mistas de
  provedores; desative-os apenas em ambientes isolados de fuzzing.

## 4. Estágio de transporte e anonimato

Use os campos `rollout_phase`, `anonymity_policy` e `transport_policy` para
representar a postura de privacidade:

- Prefira `rollout_phase="snnet-5"` e permita que a política de anonimato padrão
  acompanhe os marcos do SNNet-5. Substitua via `anonymity_policy_override` apenas
  quando a governança emitir uma diretiva assinada.
- Mantenha `transport_policy="soranet-first"` como base enquanto SNNet-4/5/5a/5b/6a/7/8/12/13 estiverem 🈺
  (veja `roadmap.md`). Use `transport_policy="direct-only"` somente para downgrades
  documentados ou exercícios de compliance e aguarde a revisão de cobertura PQ antes
  de promover `transport_policy="soranet-strict"` — esse nível falha rápido se apenas
  relés clássicos permanecerem.
- `write_mode="pq-only"` só deve ser imposto quando cada caminho de escrita (SDK,
  orquestrador, tooling de governança) puder satisfazer requisitos PQ. Durante
  rollouts, mantenha `write_mode="allow-downgrade"` para que respostas de emergência
  possam usar rotas diretas enquanto a telemetria sinaliza o downgrade.
- A seleção de guards e o staging de circuitos dependem do diretório SoraNet.
  Forneça o snapshot assinado de `relay_directory` e persista o cache de `guard_set`
  para que o churn de guards permaneça na janela de retenção acordada. A impressão
  digital do cache registrada por `sorafs_cli fetch` faz parte da evidência de rollout.

## 5. Hooks de downgrade e compliance

Dois subsistemas do orquestrador ajudam a impor a política sem intervenção manual:

- **Remediação de downgrade** (`downgrade_remediation`): monitora eventos
  `handshake_downgrade_total` e, após o `threshold` configurado ser excedido em
  `window_secs`, força o proxy local para `target_mode` (metadata-only por padrão).
  Mantenha os padrões (`threshold=3`, `window=300`, `cooldown=900`) a menos que
  revisões de incidentes indiquem outro padrão. Documente qualquer override no
  log de rollout e garanta que os dashboards acompanhem `sorafs_proxy_downgrade_state`.
- **Política de compliance** (`compliance`): carve-outs de jurisdição e manifesto
  fluem por listas de opt-out geridas pela governança. Nunca insira overrides ad hoc
  no bundle de configuração; em vez disso, solicite uma atualização assinada de
  `governance/compliance/soranet_opt_outs.json` e reimplante o JSON gerado.

Para ambos os sistemas, persista o bundle de configuração resultante e inclua-o
nas evidências de release para que auditores possam rastrear como as reduções foram
acionadas.

## 6. Telemetria e dashboards

Antes de ampliar o rollout, confirme que os seguintes sinais estão ativos no
ambiente alvo:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  deve estar em zero após a conclusão do canary.
- `sorafs_orchestrator_retries_total` e
  `sorafs_orchestrator_retry_ratio` — devem se estabilizar abaixo de 10 % durante
  o canary e permanecer abaixo de 5 % após GA.
- `sorafs_orchestrator_policy_events_total` — valida que a etapa de rollout esperada
  está ativa (label `stage`) e registra brownouts via `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — acompanham o suprimento de relés PQ em
  relação às expectativas da política.
- Alvos de log `telemetry::sorafs.fetch.*` — devem ser enviados ao agregador de logs
  compartilhado com buscas salvas para `status=failed`.

Carregue o dashboard Grafana canônico em
`dashboards/grafana/sorafs_fetch_observability.json` (exportado no portal em
**SoraFS → Fetch Observability**) para que os seletores de região/manifesto, o
heatmap de retries por provedor, os histogramas de latência de chunks e os
contadores de stall correspondam ao que o SRE revisa durante os burn-ins. Conecte
as regras do Alertmanager em `dashboards/alerts/sorafs_fetch_rules.yml` e valide a
sintaxe do Prometheus com `scripts/telemetry/test_sorafs_fetch_alerts.sh` (o helper
executa `promtool test rules` localmente ou em Docker). As passagens de alertas
exigem o mesmo bloco de roteamento que o script imprime para que operadores possam
anexar a evidência ao ticket de rollout.

### Fluxo de burn-in de telemetria

O item de roadmap **SF-6e** exige um burn-in de telemetria de 30 dias antes de
alternar o orquestrador multi-origem para seus padrões GA. Use os scripts do
repositório para capturar um bundle de artefatos reproduzível para cada dia da
janela:

1. Execute `ci/check_sorafs_orchestrator_adoption.sh` com as variáveis de burn-in
   configuradas. Exemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   O helper reproduz `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   grava `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` e `adoption_report.json` em
   `artifacts/sorafs_orchestrator/<timestamp>/`, e impõe um número mínimo de
   provedores elegíveis via `cargo xtask sorafs-adoption-check`.
2. Quando as variáveis de burn-in estão presentes, o script também emite
   `burn_in_note.json`, capturando o label, o índice do dia, o id do manifesto,
   a fonte de telemetria e os digests dos artefatos. Anexe esse JSON ao log de
   rollout para deixar claro qual captura satisfez cada dia da janela de 30 dias.
3. Importe o dashboard Grafana atualizado (`dashboards/grafana/sorafs_fetch_observability.json`)
   para o workspace de staging/produção, marque-o com o label de burn-in e confirme
   que cada painel exibe amostras para o manifesto/região em teste.
4. Execute `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ou `promtool test rules …`)
   sempre que `dashboards/alerts/sorafs_fetch_rules.yml` mudar para documentar que
   o roteamento de alertas corresponde às métricas exportadas durante o burn-in.
5. Arquive o snapshot do dashboard, a saída do teste de alertas e o tail de logs
   das buscas `telemetry::sorafs.fetch.*` junto aos artefatos do orquestrador para
   que a governança possa reproduzir a evidência sem extrair métricas de sistemas
   em produção.

## 7. Checklist de rollout

1. Regenere scoreboards em CI usando a configuração candidata e capture os
   artefatos sob controle de versão.
2. Execute o fetch determinístico dos fixtures em cada ambiente (lab, staging,
   canary, produção) e anexe os artefatos `--scoreboard-out` e `--json-out` ao
   registro de rollout.
3. Revise dashboards de telemetria com o engenheiro de plantão, garantindo que
   todas as métricas acima tenham amostras ao vivo.
4. Registre o caminho final de configuração (geralmente via `iroha_config`) e o
   commit git do registro de governança usado para anúncios e compliance.
5. Atualize o tracker de rollout e notifique as equipes de SDK sobre os novos
   padrões para manter as integrações de clientes alinhadas.

Seguir este guia mantém os deployments do orquestrador determinísticos e
passíveis de auditoria, enquanto fornece ciclos de feedback claros para ajustar
orçamentos de retries, capacidade de provedores e postura de privacidade.
