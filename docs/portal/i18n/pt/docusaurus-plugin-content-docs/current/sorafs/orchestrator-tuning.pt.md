---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ajuste do orquestrador
título: Rollout e ajuste do orquestrador
sidebar_label: Ajuste do orquestrador
descrição: Padrões práticos, orientação de ajuste e checkpoints de auditoria para levar o orquestrador multi-origem à GA.
---

:::nota Fonte canônica
Espelha `docs/source/sorafs/developer/orchestrator_tuning.md`. Mantenha as duas verificações verificadas até que a documentação alternativa seja apresentada.
:::

# Guia de rollout e ajuste do orquestrador

Este guia se baseia na [referência de configuração](orchestrator-config.md) e no
[runbook de implementação multi-origem](multi-source-rollout.md). Ele explica
como ajustar o orquestrador para cada fase de implementação, como interpretar os
artefatos do placar e quais sinais de telemetria devem estar prontos antes
de ampliar o tráfego. Aplique as recomendações de forma consistente na CLI, nos
SDKs e na automação para que cada um siga a mesma política de busca determinística.

## 1. Conjuntos de parâmetros básicos

Parte de um modelo de configuração compartilhada e ajuste de um pequeno conjunto
de botões à medida que o lançamento avança. A tabela abaixo captura os valores
recomendados para as fases mais comuns; os valores não listados voltam aos
padrões de `OrchestratorConfig::default()` e `FetchOptions::default()`.

| Fase | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Notas |
|------|-----------------|------------------------------------------|---------------------------------------|-------------------------------------------|---------------------------------------------------|-------|
| **Laboratório / CI** | `3` | `2` | `2` | `2500` | `300` | Um limite de latência e uma janela de graça estreita expõem telemetria ruidosa rapidamente. Mantenha tentativas baixas para revelar manifestos inválidos mais cedo. |
| **Encenação** | `4` | `3` | `3` | `4000` | `600` | Espelha os padrões de produção deixando folga para pares exploratórios. |
| **Canário** | `6` | `3` | `3` | `5000` | `900` | Igual aos padrões; defina `telemetry_region` para que os dashboards possam separar o tráfego canário. |
| **Disponibilidade geral** | `None` (usar todos os elegíveis) | `4` | `4` | `5000` | `900` | Aumente os limites de nova tentativa e falha para absorver falhas transitórias enquanto as auditorias continuam reforçando o determinismo. |

- `scoreboard.weight_scale` permanece no padrão `10_000` a menos que um sistema downstream exija outra resolução inteira. Aumentar a escala não altera a ordenação dos provedores; apenas emite uma distribuição de créditos mais densa.
- Ao migrar entre fases, persista o pacote JSON e use `--scoreboard-out` para que a trilha de auditoria registre o conjunto exato de parâmetros.

## 2. Higiene do placar

O placar combina requisitos do manifesto, anúncios de provedores e telemetria.
Antes de avançar:1. **Valide a frescura da telemetria.** Garanta que os snapshots referenciados por
   `--telemetry-json` foram capturados dentro da janela de graça configurada. Entradas
   mais antigos que `telemetry_grace_secs` falham com `TelemetryStale { last_updated }`.
   Trate isso como um bloqueio rígido e atualize a exportação de telemetria antes de seguir.
2. **Inspeção razões de elegibilidade.** Persista aos artesãos via
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Cada entrada
   traz um bloco `eligibility` com a causa exata da falha. Não sobreescreva
   desajustes de capacidade ou anúncios expirados; Corrija a carga útil upstream.
3. **Revise mudanças de peso.** Compare o campo `normalised_weight` com o release
   anterior. Mudanças >10% correlacionadas com alterações deliberadas em anúncios
   ou telemetria e precisam ser marcas registradas no log de rollout.
4. **Arquive artistas.** Configure `scoreboard.persist_path` para cada execução
   emita o snapshot final do scoreboard. Anexo o artigos ao registro de lançamento
   junto ao manifesto e ao pacote de telemetria.
5. **Registre evidências de mix de provedores.** Metadados de `scoreboard.json` _e_ o
   `summary.json` correspondente deve exportar `provider_count`, `gateway_provider_count`
   e o rótulo derivado `provider_mix` para que os revisores provem se a execução foi
   `direct-only`, `gateway-only` ou `mixed`. Capturas gateway reportam `provider_count=0`
   e `provider_mix="gateway-only"`, enquanto execuções erradas desabilitam contagens não
   zero para ambas as fontes. `cargo xtask sorafs-adoption-check` impor esses campos
   (e falha quando contagens/rótulos divergem), então execute-o sempre junto com
   `ci/check_sorafs_orchestrator_adoption.sh` ou seu script de captura para produção
   o pacote de evidências `adoption_report.json`. Quando os gateways Torii estiverem
   envolvidos, mantenha `gateway_manifest_id`/`gateway_manifest_cid` nos metadados do
   scoreboard para que o portão de adoção consiga correlacionar o envelope do manifesto
   com o mix de provedores capturados.

Para definições de campos, veja
`crates/sorafs_car/src/scoreboard.rs` e a estrutura de resumo do CLI exibida por
`sorafs_cli fetch --json-out`.

## Referência de flags da CLI e SDK

`sorafs_cli fetch` (ver `crates/sorafs_car/src/bin/sorafs_cli.rs`) e o invólucro
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) juntas
a mesma superfície de configuração do orquestrador. Use os seguintes sinalizadores ao
capturar evidências de rollout ou ao reproduzir os fixtures canônicos:

Referência compartilhada de flags multi-origem (mantenha a ajuda da CLI e os documentos
editados sincronizados apenas este arquivo):- `--max-peers=<count>` limita quantos provedores elegíveis sobreviveram ao filtro do placar. Deixe sem configurar para fazer streaming de todos os provedores elegíveis e definir `1` apenas quando estiver exercendo deliberadamente o fallback de fonte única. Espelha o botão `maxPeers` nos SDKs (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` encaminha para o limite de tentativas por pedaço aplicado por `FetchOptions`. Utilize uma tabela de implementação no guia de ajuste para os valores recomendados; execuções de CLI que coletam evidências devem respeitar os padrões dos SDKs para manter a paridade.
- `--telemetry-region=<label>` rotula as séries Prometheus `sorafs_orchestrator_*` (e relés OTLP) com um rótulo de região/ambiente para que os dashboards separem tráfego de laboratório, staging, canary e GA.
- `--telemetry-json=<path>` injeta o snapshot referenciado pelo scoreboard. Persista o JSON ao lado do placar para que os auditores possam reproduzir a execução (e para que `cargo xtask sorafs-adoption-check --require-telemetry` prove qual stream OTLP alimentou a captura).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) habilitam os ganchos da ponte do observador. Quando definido, o orquestrador transmite chunks através do proxy Norito/Kaigi local para que clientes de navegador, guard caches e salas Kaigi recebam os mesmos recibos emitidos por Rust.
- `--scoreboard-out=<path>` (opcionalmente com `--scoreboard-now=<unix_secs>`) persiste o instantâneo de elegibilidade para auditores. Sempre emparelhado o JSON persistido com os artefatos de telemetria e manifesto referenciados no ticket de lançamento.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` aplica ajustes determinísticos sobre metadados de anúncios. Use essas flags apenas para ensaios; Os downgrades de produção devem ser passados ​​por artistas de governança para que cada um não aplique o mesmo pacote de política.
- `--provider-metrics-out` / `--chunk-receipts-out` retêm métricas de saúde por provedor e recibos de pedaços referenciados pela lista de verificação de implementação; anexo ambos os artistas ao registrador a evidência de adoção.

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
Rust (`crates/iroha/src/client.rs`), sem ligações JS
(`javascript/iroha_js/src/sorafs.js`) e sem SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Mantenha esses ajudantes em
sincronia com os padrões da CLI para que os operadores possam copiar políticas entre
proteção sem camadas de tradução sob medida.

## 3. Ajuste da política de busca

`FetchOptions` controla nova tentativa, concorrência e seleção. Ao ajustar:- **Retries:** Elevar `per_chunk_retry_limit` acima de `4` aumenta o tempo de
  recuperação, mas pode mascarar falhas de provedores. Prefira manter `4` como
  teto e confiança na distribuição de provedores para exportação dos fracos.
- **Limiar falhas:** `provider_failure_threshold` define quando um provedor é
  desativado pelo restante da sessão. Alinhe esse valor à política de nova tentativa:
  um limite menor que o orçamento de nova tentativa força o orquestrador a ejetar um par
  antes de esgotar todas as tentativas.
- **Concorrência:** Deixe `global_parallel_limit` sem definir (`None`) a menos que
  um ambiente específico não consegue saturar as faixas anunciadas. Quando definido,
  garanta que o valor seja ≤ à soma dos orçamentos de streams dos provedores para
  evitar a fome.
- **Toggles de verificação:** `verify_lengths` e `verify_digests` devem permanecer
  habilitados em produção. Eles garantem o determinismo quando há frotas erradas
  provedores; desative-os apenas em ambientes isolados de fuzzing.

## 4. Estágio de transporte e anonimato

Utilize os campos `rollout_phase`, `anonymity_policy` e `transport_policy` para
representar uma postura de privacidade:

- Prefira `rollout_phase="snnet-5"` e permita que a política de anonimato padrão
  acompanhe os marcos do SNNet-5. Substituição via `anonymity_policy_override` apenas
  quando a governança emitir uma cláusula assinada.
- Mantenha `transport_policy="soranet-first"` como base enquanto SNNet-4/5/5a/5b/6a/7/8/12/13 estiverem 🈺
  (veja `roadmap.md`). Use `transport_policy="direct-only"` somente para downgrades
  documentados ou exercícios de compliance e aguarde a revisão de cobertura PQ antes
  de promoção `transport_policy="soranet-strict"` — esse nível falha rápido se apenas
  relés clássicos permanecerem.
- `write_mode="pq-only"` só deve ser imposto quando cada caminho de escrita (SDK,
  orquestrador, ferramentas de governança) podem satisfazer os requisitos PQ. Durante
  rollouts, mantenha `write_mode="allow-downgrade"` para que respostas de emergência
  pode usar rotas diretas enquanto a telemetria sinaliza ou downgrade.
- A seleção de guardas e o estadiamento de circuitos dependentes do diretório SoraNet.
  Forneça o snapshot assinado de `relay_directory` e persista o cache de `guard_set`
  para que a agitação dos guardas permaneça na janela de retenção acordada. A impressão
  digital do cache registrado por `sorafs_cli fetch` faz parte da evidência de rollout.

## 5. Ganchos de downgrade e compliance

Dois subsistemas do orquestrador ajudam a impor a política sem intervenção manual:- **Remediação de downgrade** (`downgrade_remediation`): monitora eventos
  `handshake_downgrade_total` e, após o `threshold` configurado ser substituído em
  `window_secs`, força o proxy local para `target_mode` (somente metadados por padrão).
  Mantenha os padrões (`threshold=3`, `window=300`, `cooldown=900`) pelo menos que
  revisões de incidentes indiquem outro padrão. Documente qualquer substituição não
  log de rollout e garanta que os dashboards acompanhem `sorafs_proxy_downgrade_state`.
- **Política de compliance** (`compliance`): exclusões de jurisdição e manifesto
  fluem por listas de opt-out geridas pela governança. Nunca insira substituições ad hoc
  nenhum pacote de configuração; em vez disso, solicite uma atualização assinada de
  `governance/compliance/soranet_opt_outs.json` e reimplante o JSON gerado.

Para ambos os sistemas, persiste o pacote de configuração resultante e incluindo-o
nas evidências de liberação para que os auditores possam rastrear como as reduções foram
acionadas.

## 6. Telemetria e dashboards

Antes de ampliar o rollout, confirme que os seguintes sinais estão ativos no
ambiente alvo:

-`sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  deve estar em zero após a conclusão do canário.
-`sorafs_orchestrator_retries_total` e
  `sorafs_orchestrator_retry_ratio` — deve ser estabilizado abaixo de 10% durante
  o canário e permanência abaixo de 5% após GA.
- `sorafs_orchestrator_policy_events_total` — valida que a etapa de implementação esperada
  está ativado (etiqueta `stage`) e registra quedas de energia via `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — acompanha o fornecimento de relés PQ em
  relação às expectativas da política.
- Alvos de log `telemetry::sorafs.fetch.*` — devem ser enviados ao agregador de logs
  compartilhado com buscas salvas para `status=failed`.

Carregue o painel Grafana canônico em
`dashboards/grafana/sorafs_fetch_observability.json` (exportado no portal em
**SoraFS → Fetch Observability**) para que os seletores de região/manifesto, o
heatmap de tentativas por provedor, os histogramas de latência de chunks e os
contadores de stall correspondem ao que o SRE revisa durante os burn-ins. Conecte-se
as regras do Alertmanager em `dashboards/alerts/sorafs_fetch_rules.yml` e valide a
sintaxe do Prometheus com `scripts/telemetry/test_sorafs_fetch_alerts.sh` (o helper
executa `promtool test rules` localmente ou em Docker). As passagens de alertas
desativar o mesmo bloco de roteamento que o script imprime para que os operadores possam
fixação a evidência ao ticket de rollout.

### Fluxo de burn-in de telemetria

O item de roadmap **SF-6e** exige um burn-in de telemetria de 30 dias antes de
alternativa ou orquestrador multi-origem para seus padrões GA. Use scripts de sistema operacional para fazer
repositório para capturar um pacote de artistas reproduzíveis para cada dia da
janela:

1. Execute `ci/check_sorafs_orchestrator_adoption.sh` com as variáveis de burn-in
   configurações. Exemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```O ajudante reproduz `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   gravação `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` e `adoption_report.json` em
   `artifacts/sorafs_orchestrator/<timestamp>/`, e impor um número mínimo de
   provedores elegíveis via `cargo xtask sorafs-adoption-check`.
2. Quando as variáveis de burn-in estão presentes, o script também emite
   `burn_in_note.json`, capturando o rótulo, o índice do dia, o id do manifesto,
   a fonte de telemetria e os resumos dos artistas. Anexo esse JSON ao log de
   rollout para deixar claro qual captura satisfatória cada dia da janela de 30 dias.
3. Importe o painel Grafana atualizado (`dashboards/grafana/sorafs_fetch_observability.json`)
   para o espaço de trabalho de encenação/produção, marque-o com o rótulo de burn-in e confirme
   que cada painel exibe amostras para o manifesto/região em teste.
4. Execute `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ou `promtool test rules …`)
   sempre que `dashboards/alerts/sorafs_fetch_rules.yml` mudar para documentar que
   o roteamento de alertas corresponde às métricas exportadas durante o burn-in.
5. Arquivar o snapshot do dashboard, a saída do teste de alertas e o final dos logs
   das buscas `telemetry::sorafs.fetch.*` junto aos artistas do orquestrador para
   que a governança possa produzir evidências sem extrair métricas de sistemas
   em produção.

## 7. Lista de verificação de implementação

1. Regenerar scoreboards em CI usando a configuração candidata e capturar o sistema operacional
   artistas sob controle de versão.
2. Execute a busca determinística dos fixtures em cada ambiente (laboratório, staging,
   canary, produção) e anexo dos artistas `--scoreboard-out` e `--json-out` ao
   registro de lançamento.
3. Revisar dashboards de telemetria com o engenheiro de plantação, garantindo que
   todas as análises acima têm amostras ao vivo.
4. Registre o caminho final de configuração (geralmente via `iroha_config`) e o
   commit git do registro de governança usado para anúncios e compliance.
5. Atualize o tracker de rollout e notifique as equipes de SDK sobre os novos
   padrões para manter as integrações de clientes homologadas.

Seguir este guia mantém as implantações do orquestrador determinísticos e
passíveis de auditorias, enquanto fornece ciclos de feedback claros para ajustar
orçamentos de tentativas, capacidade de provedores e postura de privacidade.