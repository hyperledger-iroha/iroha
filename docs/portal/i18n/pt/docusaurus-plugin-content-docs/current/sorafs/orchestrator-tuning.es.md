---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ajuste do orquestrador
título: Despliegue e ajuste do orquestrador
sidebar_label: Ajuste do orquestrador
descrição: Valores predeterminados práticos, guia de ajuste e pontos de auditoria para levar o orquestrador multi-origem ao GA.
---

:::nota Fonte canônica
Reflexo `docs/source/sorafs/developer/orchestrator_tuning.md`. Mantenha ambas as cópias alinhadas até que o conjunto de documentação herdado seja retirado.
:::

# Guia de despliegue e ajuste do orquestrador

Este guia está baseado na [referência de configuração](orchestrator-config.md) e o
[runbook de despliegue multi-origen](multi-source-rollout.md). Explicação
como ajustar o orquestrador para cada fase de despliegue, como interpretar os
artefatos do placar e quais sinais de telemetria devem estar listados antes
ampliar o tráfego. Aplique as recomendações de forma consistente na
CLI, o SDK e a automação para que cada nó siga a mesma política de
buscar determinista.

## 1. Conjuntos de parâmetros base

Parte de uma planta de configuração compartilhada e ajusta um conjunto pequeno
de perigos na medida em que progride o despliegue. La tabla siguiente recoge los
valores recomendados para fases mais comunes; os valores no listado vuelven
nos predeterminados em `OrchestratorConfig::default()` e `FetchOptions::default()`.

| Fase | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Notas |
|------|-----------------|------------------------------------------|---------------------------------------|-------------------------------------------|---------------------------------------------------|-------|
| **Laboratório / CI** | `3` | `2` | `2` | `2500` | `300` | Um limite de latência e uma janela de graça restrita expõem a telemetria ruidosa rapidamente. Mantenha as intenções baixas para descobrir os manifestos inválidos antes. |
| **Encenação** | `4` | `3` | `3` | `4000` | `600` | Reflita os valores de produção deixando margem para pares exploratórios. |
| **Canário** | `6` | `3` | `3` | `5000` | `900` | Igual aos valores por defeito; configure `telemetry_region` para que os dashboards possam segmentar o tráfego canário. |
| **Disponibilidade geral** | `None` (usar todos os elegíveis) | `4` | `4` | `5000` | `900` | Aumentar os umbrais de reintenções e falhas para absorver falhas transitórias enquanto as auditorias continuam reforçando o determinismo. |

- `scoreboard.weight_scale` é mantido no valor predeterminado `10_000` desde que um sistema com água abaixo exija outra resolução. Aumentar a escala sem alterar a ordem dos fornecedores; só emite uma distribuição de créditos mais densa.
- Ao migrar entre fases, persista o pacote JSON e use `--scoreboard-out` para que o rastro de auditoria registre o conjunto exato de parâmetros.

## 2. Higiene do placar

O placar combina requisitos de manifestação, anúncios de provedores e telemetria.
Antes de avançar:1. **Valida a frescura da telemetria.** Certifique-se de que os instantâneos referenciados por
   `--telemetry-json` força capturada dentro da janela de graça configurada. As entradas
   mais antigo que `telemetry_grace_secs` caiu com `TelemetryStale { last_updated }`.
   Trate-o como um bloqueio rígido e atualize a exportação de telemetria antes de continuar.
2. **Inspecione os motivos de elegibilidade.** Persista os artefatos com
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Cada entrada
   inclui um bloco `eligibility` com a causa exata da falha. Não sobreescribas
   desajustes de capacidades ou anúncios expirados; Corrija a carga útil upstream.
3. **Revisar as mudanças de peso.** Comparar o campo `normalised_weight` com o
   solte anterior. Desplazamentos de peso >10% devem ser correlacionados com mudanças
   deliberados em anúncios ou telemetria e registrados no log de despliegue.
4. **Arquive os artefatos.** Configure `scoreboard.persist_path` para cada um
   a execução emite o instantâneo final do placar. Adjunta o artefato ao registro
   de liberação junto com o manifesto e o pacote de telemetria.
5. **Registre a evidência da mistura de fornecedores.** Os metadados de `scoreboard.json`
   e o `summary.json` corresponde ao expositor `provider_count`,
   `gateway_provider_count` e a etiqueta derivada `provider_mix` para os revisores
   Verifique se a execução foi `direct-only`, `gateway-only` ou `mixed`. As capturas de
   gateway reportan `provider_count=0` e `provider_mix="gateway-only"`, enquanto eles
   ejecuciones mixtas exigem conteúdo sem zero para ambas as origens. `cargo xtask sorafs-adoption-check`
   impor esses campos (e falhar se os conteúdos/etiquetas não coincidirem), assim que ejecutá-lo
   sempre junto com `ci/check_sorafs_orchestrator_adoption.sh` ou seu script de captura para
   produza o pacote de evidências `adoption_report.json`. Quando haya gateways Torii,
   conservar `gateway_manifest_id`/`gateway_manifest_cid` nos metadados do placar
   para que a porta de adoção possa correlacionar a sobre a manifestação com a
   mistura de provedores capturados.

Para definições detalhadas de campos, consulte
`crates/sorafs_car/src/scoreboard.rs` e a estrutura de currículo da CLI exposta por
`sorafs_cli fetch --json-out`.

## Referência de flags de CLI e SDK

`sorafs_cli fetch` (ver `crates/sorafs_car/src/bin/sorafs_cli.rs`) e o invólucro
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) compartilha la
mesma superfície de configuração do orquestrador. Usa as seguintes bandeiras al
capturar evidências de despliegue ou reproduzir os jogos canônicos:

Referência compartilhada de bandeiras de origem múltipla (mantén a ajuda de CLI e os documentos em
edição sincronizada apenas neste arquivo):- `--max-peers=<count>` limita quantos provadores elegíveis sobreviveram no filtro do placar. Deixe-o sem configurar para transmitir de todos os provedores elegíveis e coloque-o em `1` somente quando o fallback de uma única fonte for intencionalmente executado. Reflita sobre o perigo `maxPeers` no SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` reenvia o limite de reintenções por parte do aplicativo `FetchOptions`. Use a tabela de lançamento no guia de ajuste para valores recomendados; As execuções da CLI que coletam evidências devem coincidir com os valores defeituosos do SDK para manter a paridade.
- `--telemetry-region=<label>` etiqueta a série Prometheus `sorafs_orchestrator_*` (e os relés OTLP) com uma etiqueta de região/entorno para que os painéis separem o tráfego de laboratório, teste, canário e GA.
- `--telemetry-json=<path>` inyecta o instantâneo referenciado pelo placar. Persista o JSON junto com o placar para que os auditores possam reproduzir a execução (e para que `cargo xtask sorafs-adoption-check --require-telemetry` teste o fluxo OTLP que alimentou a captura).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) habilita os ganchos da ponte do coletor. Quando configurado, o orquestrador transmite pedaços através do proxy local Norito/Kaigi para que os clientes do navegador, guardem caches e salas Kaigi recebam os mesmos recibos emitidos por Rust.
- `--scoreboard-out=<path>` (opcionalmente com `--scoreboard-now=<unix_secs>`) persiste o instantâneo de elegibilidade para os auditores. Compare sempre o JSON persistido com os artefatos de telemetria e se manifeste referenciado no ticket de liberação.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` aplica configurações deterministas sobre os metadados de anúncios. Usamos estas bandeiras apenas para ensaios; as degradações na produção devem passar por artefatos de governança para que cada nodo aplique o mesmo pacote de políticas.
- `--provider-metrics-out` / `--chunk-receipts-out` conservam as métricas de saúde do fornecedor e os recibos de pedaços referenciados pela lista de verificação de implementação; adicione ambos os artefatos para apresentar a evidência de adoção.

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

O SDK consome a mesma configuração por meio de `SorafsGatewayFetchOptions` em
o cliente Rust (`crates/iroha/src/client.rs`), as ligações JS
(`javascript/iroha_js/src/sorafs.js`) e o SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Mantenha essas ajudas em
sincronização com os valores por defeito da CLI para que os operadores possam
copiar políticas entre automatização sem capas de tradução sob medida.

## 3. Ajuste da política de busca

`FetchOptions` controla o comportamento de reintenções, concorrência e verificação.
Como ajustar:- **Reintenções:** Elevar `per_chunk_retry_limit` por mais de `4` aumenta o tempo
  de recuperação, mas pode ocultar falhas de fornecedores. Prefira mantenedor `4`
  como techo e confie na rotação de fornecedores para detectar o baixo rendimento.
- **Umbral de fallos:** `provider_failure_threshold` determina quando um provedor
  se debilita para o resto da sessão. Alinea é valor con la política de
  reintentos: um umbral menor que o pressuposto de reintentos obriga ao orquestrador
  a expulsar um par antes de agotar todos os reintentos.
- **Concorrência:** Deixe `global_parallel_limit` sem configurar (`None`) pelo menos
  que um ambiente específico não pode saturar os rangos anunciados. Quando se
  configure, certifique-se de que o valor seja ≤ na soma dos pressupostos de
  streams dos provedores para evitar a inanição.
- **Toggles de verificação:** `verify_lengths` e `verify_digests` devem permanecer
  habilitados na produção. Garantizan o determinismo quando hay flotas mixtas
  de provedores; apenas desativados em ambientes de fuzzing isolados.

## 4. Etapas de transporte e anonimato

Use os campos `rollout_phase`, `anonymity_policy` e `transport_policy` para
representando a postura de privacidade:

- Prefira `rollout_phase="snnet-5"` e permite que a política de anonimato por
  defeito siga os sucessos do SNNet-5. Sobrescrever com `anonymity_policy_override`
  somente quando o governo emite uma diretiva firmada.
- Mantenha `transport_policy="soranet-first"` como base enquanto SNNet-4/5/5a/5b/6a/7/8/12/13 está 🈺
  (ver `roadmap.md`). Usa `transport_policy="direct-only"` solo para degradações
  documentados ou simulacros de cumprimento, e aguarda a revisão da cobertura PQ
  antes de promover `transport_policy="soranet-strict"`—este nível cairá rapidamente se
  solo quedan relés clássicos.
- `write_mode="pq-only"` só deve ser imposto quando cada rota de escritura (SDK,
  orquestrador, ferramentas de governo) pode satisfazer os requisitos PQ. Durante
  os lançamentos mantêm `write_mode="allow-downgrade"` para que as respostas de
  emergência pode ser apoyarse em rotas diretas enquanto a telemetria marca la
  degradação.
- A seleção de guardas e a preparação de circuitos dependem do diretório
  de SoraNet. Proporciona o snapshot firmado de `relay_directory` e persiste
  cache de `guard_set` para que o churn de guardias se mantenha dentro do
  janela de retenção acordada. A casca do cache registrada por
  `sorafs_cli fetch` forma parte da evidência de implementação.

## 5. Ganchos de degradação e cumprimento

Dois subsistemas do orquestrador ajudam a impor a política sem manual de intervenção:- **Remediação de degradações** (`downgrade_remediation`): monitorea eventos
  `handshake_downgrade_total` e, depois que o `threshold` foi configurado
  dentro de `window_secs`, força o proxy local para `target_mode` (por
  defeito somente metadados). Mantenha os valores predeterminados (`threshold=3`,
  `window=300`, `cooldown=900`) salva que os postmortems indicam um patrono
  distinto. Documente qualquer substituição no log de implementação e certifique-se de que
  os painéis seguem `sorafs_proxy_downgrade_state`.
- **Política de cumprimento** (`compliance`): los carve-outs por jurisdicción y
  manifesta-se através de listas de exclusão administradas pelo governo.
  Nunca insere substituições ad hoc no pacote de configuração; em seu lugar,
  solicitar uma atualização firmada de
  `governance/compliance/soranet_opt_outs.json` e desinstale o JSON gerado.

Para ambos os sistemas, persista o pacote de configuração resultante e inclua-o
nas evidências de lançamento para que os auditores possam rastrear como se
ative as reduções.

## 6. Telemetria e painéis

Antes de ampliar o despliegue, confirme que as seguintes sinais estão ativadas
no entorno objetivo:

-`sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  deve ser zero depois de completar o canário.
-`sorafs_orchestrator_retries_total` e
  `sorafs_orchestrator_retry_ratio` — deve ser estabilizado por baixo de 10%
  durante o canário e mantido por baixo de 5% tras GA.
- `sorafs_orchestrator_policy_events_total` — valida a etapa de implementação
  esperado está ativado (etiqueta `stage`) e registra quedas de energia via `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — rastreia o suprimento de relés PQ
  frente às expectativas da política.
- Objetivos de log `telemetry::sorafs.fetch.*` — deve fluir no agregador de logs
  compartilhado com buscas guardadas para `status=failed`.

Carregue o painel canônico de Grafana desde
`dashboards/grafana/sorafs_fetch_observability.json` (exportado no portal
abaixo **SoraFS → Fetch Observability**) para os seletores de região/manifest,
o mapa de calor de reintenções por provedor, os histogramas de latência de pedaços e
Os contadores de atascos coincidem com o que revisa o SRE durante os burn-ins.
Conecte os regulamentos do Alertmanager em `dashboards/alerts/sorafs_fetch_rules.yml`
e valida a sintaxe de Prometheus com `scripts/telemetry/test_sorafs_fetch_alerts.sh`
(o auxiliar executa automaticamente `promtool test rules` localmente ou em Docker).
As transferências de alertas exigem o mesmo bloco de roteamento que imprime
o script para que os operadores possam adicionar a evidência ao ticket de lançamento.

### Fluxo de queima de telemetria

O item do roteiro **SF-6e** requer um burn-in de telemetria de 30 dias antes
de mudar o orquestrador multi-origem para seus valores GA. Usa os scripts do
repositório para capturar um pacote de artefatos reproduzíveis todos os dias no
Ventana:

1. Execute `ci/check_sorafs_orchestrator_adoption.sh` com as variáveis de
   ambiente de burn-in configurado. Exemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```El ajudante reproduz `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   descrever `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` e `adoption_report.json` baixo
   `artifacts/sorafs_orchestrator/<timestamp>/`, e impõe um número mínimo de
   provedores elegíveis mediante `cargo xtask sorafs-adoption-check`.
2. Quando as variáveis de burn-in estão presentes, o script também é emitido
   `burn_in_note.json`, capturando a etiqueta, o índice do dia, o id do
   manifesto, a fonte de telemetria e os resumos dos artefatos. Adjunto
   este JSON no log de lançamento para que seja evidente o que foi capturado todos os dias
   de la ventana de 30 dias.
3. Importe a tabela Grafana atualizada (`dashboards/grafana/sorafs_fetch_observability.json`)
   no espaço de trabalho de preparação/produção, etiqueta com a etiqueta de gravação
   e confirme que cada painel mostra as demonstrações para o manifesto/região em teste.
4. Ejecuta `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ou `promtool test rules …`)
   quando mudar `dashboards/alerts/sorafs_fetch_rules.yml` para documentar que
   o roteamento de alertas coincide com as informações exportadas durante o burn-in.
5. Arquive o instantâneo do painel, a saída do teste de alertas e o
   cauda dos troncos das buscas `telemetry::sorafs.fetch.*` junto com os
   artefatos do orquestrador para que o governo possa reproduzir
   evidência sem extraer métricas de sistemas en vivo.

## 7. Lista de verificação de implementação

1. Regenera os placares no CI usando a configuração candidata e captura
   os artefatos sob controle de versões.
2. Execute o determinista de busca de luminárias em cada ambiente (laboratório, palco,
   canary, produção) e adjunta aos artefatos `--scoreboard-out` e `--json-out`
   no registro de lançamento.
3. Revise os painéis de telemetria com o engenheiro de plantão, garantindo que
   todas as análises anteriores têm demonstrações ao vivo.
4. Registre a rota de configuração final (normalmente via `iroha_config`) e
   commit git do registro de governo usado para anúncios e cumprimentos.
5. Atualize o rastreador de implementação e notifique os equipamentos do SDK sobre os
   novos valores por defeito para que as integrações de clientes sejam mantidas
   alineadas.

Seguindo este guia, mantenha os despliegues do orquestrador determinista e
auditáveis, enquanto aporta ciclos de retroalimentação claros para ajustar
presupostos de reintenções, capacidade de provedores e postura de privacidade.