---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ajuste do orquestrador
título: Déploiement et réglage de l’orchestrateur
sidebar_label: Réglage do orquestrador
descrição: Valeurs par défaut pratiques, conselhos de ajuste e pontos de auditoria para amener l'orchestrateur multi-source em GA.
---

:::nota Fonte canônica
Reflete `docs/source/sorafs/developer/orchestrator_tuning.md`. Certifique-se de que as duas cópias estejam alinhadas até que a documentação herdada seja retirada.
:::

# Guia de implantação e ajuste do orquestrador

Este guia é acessado na [referência de configuração](orchestrator-config.md) e no
[runbook de implantação multifonte](multi-source-rollout.md). A explicação
comentar ajustar o orquestrador para cada fase de implantação, comentar intérprete
os artefatos do placar e os sinais de televisão devem estar no lugar
antes de aumentar o tráfego. Aplique estas recomendações de maneira coerente em
a CLI, o SDK e a automação para que cada vez não siga a mesma política de
buscar determinado.

## 1. Jogos de parâmetros básicos

Parte de um modelo de configuração compartilhada e ajuste de um pequeno conjunto de configurações
na pele e na medida da implantação. Le tableau ci-dessous captura os valores
recomendado para as fases mais atuais; os valores não listados são retomados
nos valores padrão de `OrchestratorConfig::default()` e `FetchOptions::default()`.

| Fase | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Notas |
|-------|-----------------|------------------------------------------|---------------------------------------|-------------------------------------------|--------------------------------------------------|-------|
| **Laboratório / CI** | `3` | `2` | `2` | `2500` | `300` | Um teto de latência serrado e uma janela de graça cortada rapidamente evidenciaram uma transmissão brusca. Gardez des retries bas pour révéler les manifestes invalides plus tot. |
| **Encenação** | `4` | `3` | `3` | `4000` | `600` | Reflita os valores de produção em todas as margens para os pares exploratórios. |
| **Canários** | `6` | `3` | `3` | `5000` | `900` | Corresponde aos valores por padrão; defina `telemetry_region` para permitir que os painéis de controle girem no tráfego canari. |
| **Disponibilidade geral** | `None` (usar todos os elegíveis) | `4` | `4` | `5000` | `900` | Aumente suas tentativas e verificações para absorver todas as falhas transitórias e preservar o determinismo por meio da auditoria. |

- `scoreboard.weight_scale` permanece no valor padrão `10_000`, exceto se um sistema atual precisar de outra resolução inteira. Aumente a échelle ne change pas l'ordre des fournisseurs; este produto simplifica uma distribuição de créditos mais densa.
- Ao passar uma fase para outra, persista o pacote JSON e use `--scoreboard-out` para que a pista de auditoria registre o jogo de parâmetros exato.

## 2. Higiene do placarO placar combina as exigências do manifesto, os anúncios dos fornecedores e a transmissão.
Avant d’avancer:

1. **Valide o fraîcheur da télémétrie.** Certifique-se de que os snapshots referenciados por
   `--telemetry-json` foi capturado na janela de graça configurada. As entradas
   mais antigos que `telemetry_grace_secs` foram ouvidos com `TelemetryStale { last_updated }`.
   Traitez cela como uma parada durante e rafraîchissez a exportação de télémétrie antes de continuar.
2. **Inspecione as razões de elegibilidade.** Mantenha os artefatos via
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Entrada Chaque
   transporte um bloco `eligibility` com a causa exata do cheque. Ne contornonez
   deixe os cartões de capacidade ou os anúncios expirados; corrija o valor da carga útil.
3. **Recupere os cartões de peso.** Compare o campeão `normalised_weight` com
   lançamento anterior. As variações >10% devem corresponder às alterações
   voluntários de anúncios ou de telemetria e são enviados para o diário de implantação.
4. **Arquive os artefatos.** Configure `scoreboard.persist_path` para cada execução
   emita o instantâneo final do placar. Anexe o artefato ao dossiê de lançamento
   com o manifesto e o pacote de televisão.
5. **Consigne la preuve du mix fournisseurs.** Os metadados de `scoreboard.json` _et_ le
   `summary.json` correspondente doivent expositor `provider_count`,
   `gateway_provider_count` e a etiqueta derivada `provider_mix` para os refletores
   Você pode comprovar se a execução foi feita `direct-only`, `gateway-only` ou `mixed`. Les
   captura gateway doivent relator `provider_count=0` mais `provider_mix="gateway-only"`,
   portanto, as execuções mistas exigem contas não nulas para as duas fontes.
   `cargo xtask sorafs-adoption-check` impor ces champs (e ecoar si les comptes/labels
   divergente), então execute-o sempre com `ci/check_sorafs_orchestrator_adoption.sh`
   ou seu script de captura para produzir o pacote de evidências `adoption_report.json`.
   Quando os gateways Torii são implícitos, preserve `gateway_manifest_id`/`gateway_manifest_cid`
   nos metadados do placar para que o portão de adoção possa corrigir o envelope
   você se manifesta com o mix de fornecedores capturados.

Para definições de campos detalhados, veja
`crates/sorafs_car/src/scoreboard.rs` e a estrutura do currículo CLI exposta por
`sorafs_cli fetch --json-out`.

## Referência de sinalizadores CLI e SDK

`sorafs_cli fetch` (veja `crates/sorafs_car/src/bin/sorafs_cli.rs`) e o invólucro
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) compartilhando o mesmo
superfície de configuração do orquestrador. Use as seguintes bandeiras ao longo do
capture as previsões de implantação ou rejouer les fixtures canonices:

Referência compartilhada de flags multi-source (gardez a ajuda CLI e os documentos sincronizados em
editar apenas este arquivo):- `--max-peers=<count>` limita o número de fornecedores qualificados que passam pelo filtro do placar. Deixe o vídeo para o streamer a partir de todos os fornecedores qualificados, usando `1` apenas para exercer voluntariamente a fonte mono substituta. Reflita a chave `maxPeers` no SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` transmite o limite de tentativas por pedaço aplicado por `FetchOptions`. Utilize a tabela de implementação do guia de ajuste para os valores recomendados; As execuções CLI que coletam testes devem corresponder aos valores padrão do SDK para garantir a paridade.
- `--telemetry-region=<label>` etiqueta a série Prometheus `sorafs_orchestrator_*` (e os relés OTLP) com um rótulo de região/ambiente para que os painéis distingam lab, staging, canari et GA.
- `--telemetry-json=<path>` injeta o instantâneo referenciado no placar. Persista o JSON no local do placar para que os auditores possam refazer a execução (e para que `cargo xtask sorafs-adoption-check --require-telemetry` prove que o fluxo OTLP alimenta a captura).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) ativa os ganchos da ponte do observador. Quando definidos, o orquestrador difunde os pedaços por meio do proxy Norito/Kaigi local para que os clientes naveguem, guardem caches e quartos Kaigi reconheçam os mesmos recursos que Rust.
- `--scoreboard-out=<path>` (eventualmente com `--scoreboard-now=<unix_secs>`) mantém o instantâneo de elegibilidade para os auditores. Associe sempre o JSON persistente aos artefatos de telefonia e ao manifesto referenciado no ticket de lançamento.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` aplica ajustes determinados acima dos meses de anúncio. Use essas bandeiras exclusivamente para as repetições; os rebaixamentos da produção devem passar pelos artefatos de governo para que cada um não aplique o mesmo pacote de política.
- `--provider-metrics-out` / `--chunk-receipts-out` mantém as métricas de saúde pelo fornecedor e os recursos de pedaços mencionados na lista de verificação de implementação; anexe os dois artefatos ao depósito da evidência de adoção.

Exemplo (utilizando o equipamento publicado):

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

O SDK usa a mesma configuração via `SorafsGatewayFetchOptions` no
cliente Rust (`crates/iroha/src/client.rs`), as ligações JS
(`javascript/iroha_js/src/sorafs.js`) e o SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Gardez ces ajudantes alinhados
com os valores padrão da CLI para que os operadores possam copiar os
política na automatização sem sofás de tradução ad hoc.

## 3. Ajuste da política de busca

`FetchOptions` controla as novas tentativas, a concorrência e a verificação. Senhor du
regulamento:- **Retries :** aumenta `per_chunk_retry_limit` a partir de `4` ativa o tempo
  de recuperação mais risco de mascarar falhas de fornecedores. Préférez
  garder `4` como teto e computador na rotação dos fornecedores para
  expor os maus artistas.
- **Seuil d’échec :** `provider_failure_threshold` determinar quando um fornecedor
  está desativado para o resto da sessão. Alinhe este valor à política
  de retries : um seuil inferior no orçamento de retries força o orquestrador a
  ejete um par antes que todas as tentativas não sejam executadas.
- **Concorrência:** deixe `global_parallel_limit` não definido (`None`) pelo menos
  um ambiente específico não pode saturar as praias anunciadas. Lorsque
  defini, garanto que o valor é ≤ na soma dos orçamentos de streams des
  fornecedores para evitar a fome.
- **Toggles de verificação:** `verify_lengths` e `verify_digests` devem ser restaurados
  ativos em produção. Ele garante o determinismo quando as gotas
  misturas de fornecedores são ativos; não há desativação nos ambientes
  de fuzzing isolados.

## 4. Transporte e estadiamento anônimo

Use os campeões `rollout_phase`, `anonymity_policy` e `transport_policy` para
representa a postura de confidencialidade:

- Dê preferência a `rollout_phase="snnet-5"` e deixe a política de anonimato por padrão
  siga os bloqueios SNNet-5. Substitua via `anonymity_policy_override` exclusivo
  quando o governo foi assinado por uma diretiva.
- Gardez `transport_policy="soranet-first"` como base tanto que SNNet-4/5/5a/5b/6a/7/8/12/13 são 🈺
  (veja `roadmap.md`). Utilize `transport_policy="direct-only"` exclusivamente para
  rebaixa documentos ou exercícios de conformidade e assiste à revista de
  cobertura PQ antes da promoção `transport_policy="soranet-strict"` — ce niveau
  ecoou rapidamente se seus relacionamentos clássicos subsistentes.
- `write_mode="pq-only"` não deve ser aplicado quando estiver em cada caminho de escrita
  (SDK, orquestrador, ferramentas de governo) pode satisfazer as exigências PQ. Durant
  os lançamentos, gardez `write_mode="allow-downgrade"` para as respostas de urgência
  pode tocar nas rotas diretas enquanto o telefone sinaliza
  degradação.
- A seleção de guardas e a preparação de circuitos são acionadas
  repertório SoraNet. Obtenha o snapshot assinado por `relay_directory` e
  persista o cache `guard_set` para que a rotatividade de guardas permaneça na janela
  de rétention convoca. A impressão do cache registrado por `sorafs_cli fetch`
  parte da evidência de lançamento.

## 5. Ganchos de downgrade e conformidade

Dois subsistemas do orquestrador ajudam a respeitar a política sem
intervenção Manuelle:- **Remédiation des downgrades** (`downgrade_remediation`): monitora eventos
  `handshake_downgrade_total` e, após a passagem do `threshold` configurado em
  `window_secs`, força o proxy local em `target_mode` (por padrão, apenas metadados).
  Guarde os valores padrão (`threshold=3`, `window=300`, `cooldown=900`) com segurança
  se as postmortems montarem outro esquema. Documente toda a substituição no arquivo
  diário de lançamento e garantia de que os próximos painéis
  `sorafs_proxy_downgrade_state`.
- **Politique de conformité** (`compliance`): les carve-outs de juridiction et de
  manifestou-se através das listas de opt-out gerenciadas pelo governo. N'intégrez
  jamais substitua ad hoc no pacote de configuração; demandez plutôt une
  atualização do dia assinada por `governance/compliance/soranet_opt_outs.json` e reimplantação
  o JSON foi gerado.

Para os dois sistemas, persista o pacote de configuração resultante e inclua-o
aux preuves de release afin que les auditeurs puissent retracer les bascules.

## 6. Telemetria e painéis

Antes de iniciar a implementação, confirme se os sinais seguintes estão ativos no
o ambiente cible :

-`sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  faça isso a zero depois do fim do canari.
- `sorafs_orchestrator_retries_total` e outros
  `sorafs_orchestrator_retry_ratio` — doivent se estabilizador sous 10% pendente le
  canari e rester sous 5% após GA.
- `sorafs_orchestrator_policy_events_total` — valida a etapa de implementação
  está ativo (etiqueta `stage`) e registra as quedas de energia via `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — suivent l’offre de relais PQ face aux
  atenção à política.
- Códigos de registro `telemetry::sorafs.fetch.*` — devem ser enviados ao agregador
  de registros compartilhados com pesquisas salvas para `status=failed`.

Carregue o painel Grafana canonique a partir de
`dashboards/grafana/sorafs_fetch_observability.json` (exportado para o portal sob
**SoraFS → Fetch Observability**) depois dos selecionados região/manifest, o mapa de calor
das tentativas do fornecedor, dos histogramas de latência dos pedaços e dos compteurs
o bloqueio corresponde ao que o SRE examina durante os burn-ins. Racordez as regras
Alertmanager em `dashboards/alerts/sorafs_fetch_rules.yml` e valida a sintaxe
Prometheus com `scripts/telemetry/test_sorafs_fetch_alerts.sh` (o ajudante é executado
Localização `promtool test rules` ou via Docker). As transferências de alertas exigem le
mesmo bloco de roteamento que o script imprime para que os operadores possam se unir
a evidência do ticket de lançamento.

### Fluxo de trabalho de telemetria burn-in

O item do roteiro **SF-6e** exige uma queima de televisão de 30 dias antes de
basculer l’orchestrateur multi-source vers ses valeurs GA. Utilize os scripts do
referência para capturar um pacote de artefatos reproduzíveis cada dia
janela:

1. Execute `ci/check_sorafs_orchestrator_adoption.sh` com variáveis de burn-in
   define. Exemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```Le helper rejoue `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   escrito `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` e `adoption_report.json` sob
   `artifacts/sorafs_orchestrator/<timestamp>/`, e impõe um nome mínimo de
   fornecedores qualificados via `cargo xtask sorafs-adoption-check`.
2. Quando as variáveis de burn-in estão presentes, o script também foi criado
   `burn_in_note.json`, capturando o rótulo, o índice do dia, o id do manifesto,
   a fonte de telemetria e os resumos dos artefatos. Acesse este JSON no diário
   de rollout afin qu'il soit clair quelle capture a satisfait chaque jour de la
   janela de 30 dias.
3. Importe o quadro Grafana atual (`dashboards/grafana/sorafs_fetch_observability.json`)
   no espaço de trabalho de encenação/produção, marque-o com o rótulo de burn-in
   e verifique se cada painel exibe des échantillons para o manifesto/região testada.
4. Execute `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ou `promtool test rules …`)
   Todas as vezes que `dashboards/alerts/sorafs_fetch_rules.yml` foi alterado, no documento
   que o roteamento de alertas corresponda às métricas exportadas durante o burn-in.
5. Arquive o snapshot do painel, o teste de alerta e a fila de logs
   pesquise `telemetry::sorafs.fetch.*` com os artefatos do orquestrador para
   que a governança pode refazer a evidência sem extrair métricas dos sistemas ativos.

## 7. Lista de verificação de implementação

1. Gere os placares no CI com o candidato de configuração e capture-os
   artefatos sob controle de versão.
2. Execute a busca determinada de equipamentos em cada ambiente (laboratório, teste,
   canari, produção) e anexe os artefatos `--scoreboard-out` e `--json-out`
   no registro de implementação.
3. Revise os painéis de controle com o engenheiro de tráfego, em
   verifique se todas as métricas ci-dessus ont des échantillons live.
4. Registre o caminho de configuração final (souvent via `iroha_config`) e o
   commit git do registro de governo usado para anúncios e conformidade.
5. Atualize o rastreador de lançamento e informe as novas equipes do SDK
   padrão é que as integrações do cliente permanecem alinhadas.

Siga este guia para manter as implantações do orquestrador determinado e
auditáveis, tout en fournissant des boucles de rétroaction claires pour ajuster
os orçamentos de nova tentativa, a capacidade dos fornecedores e a postura de confidencialidade.