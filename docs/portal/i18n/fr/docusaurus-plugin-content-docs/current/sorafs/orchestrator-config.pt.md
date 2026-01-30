---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: orchestrator-config
title: Configuracao do orquestrador SoraFS
sidebar_label: Configuracao do orquestrador
description: Configure o orquestrador de fetch multi-origem, interprete falhas e depure a saida de telemetria.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/developer/orchestrator.md`. Mantenha ambas as copias sincronizadas ate que a documentacao alternativa seja retirada.
:::

# Guia do orquestrador de fetch multi-origem

O orquestrador de fetch multi-origem do SoraFS conduz downloads deterministas e
paralelos a partir do conjunto de provedores publicado em adverts respaldados
pela governanca. Este guia explica como configurar o orquestrador, quais sinais
de falha esperar durante rollouts e quais fluxos de telemetria expõem
indicadores de saúde.

## 1. Visao geral da configuracao

O orquestrador combina tres fontes de configuracao:

| Fonte | Proposito | Notas |
|-------|-----------|-------|
| `OrchestratorConfig.scoreboard` | Normaliza pesos de provedores, valida a frescura da telemetria e persiste o scoreboard JSON usado para auditorias. | Apoiado por `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Aplica limites de runtime (budgets de retry, limites de concorrencia, toggles de verificacao). | Mapeia para `FetchOptions` em `crates/sorafs_car::multi_fetch`. |
| Parametros de CLI / SDK | Limita o numero de peers, anexa regioes de telemetria e expõe politicas de deny/boost. | `sorafs_cli fetch` expõe esses flags diretamente; os SDKs os propagam via `OrchestratorConfig`. |

Os helpers JSON em `crates/sorafs_orchestrator::bindings` serializam a
configuracao completa em Norito JSON, tornando-a portavel entre bindings de SDKs
e automacao.

### 1.1 Exemplo de configuracao JSON

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

Persista o arquivo atraves do empilhamento usual do `iroha_config` (`defaults/`,
user, actual) para que deployments deterministas herdem os mesmos limites entre
nodos. Para um perfil de fallback direct-only alinhado ao rollout SNNet-5a,
consulte `docs/examples/sorafs_direct_mode_policy.json` e a orientacao
correspondente em `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Overrides de conformidade

A SNNet-9 integra conformidade orientada pela governanca no orquestrador. Um
novo objeto `compliance` na configuracao Norito JSON captura os carve-outs que
forcam o pipeline de fetch ao modo direct-only:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` declara os codigos ISO-3166 alpha-2 onde esta
  instancia do orquestrador opera. Os codigos sao normalizados para maiusculas
  durante o parsing.
- `jurisdiction_opt_outs` espelha o registro de governanca. Quando qualquer
  jurisdicao do operador aparece na lista, o orquestrador aplica
  `transport_policy=direct-only` e emite o motivo de fallback
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` lista digests de manifest (CIDs cegados, codificados em
  hex maiusculo). Payloads correspondentes tambem forcam agendamento direct-only
  e expõem o fallback `compliance_blinded_cid_opt_out` na telemetria.
- `audit_contacts` registra as URIs que a governanca espera que os operadores
  publiquem nos playbooks GAR.
- `attestations` captura os pacotes de conformidade assinados que sustentam a
  politica. Cada entrada define uma `jurisdiction` opcional (codigo ISO-3166
  alpha-2), um `document_uri`, o `digest_hex` canonico de 64 caracteres, o
  timestamp de emissao `issued_at_ms` e um `expires_at_ms` opcional. Esses
  artefatos alimentam o checklist de auditoria do orquestrador para que as
  ferramentas de governanca possam vincular overrides a documentacao assinada.

Forneca o bloco de conformidade via o empilhamento usual de configuracao para
que os operadores recebam overrides deterministas. O orquestrador aplica a
conformidade _depois_ dos hints de write-mode: mesmo que um SDK solicite
`upload-pq-only`, opt-outs de jurisdicao ou manifest ainda forcam o transporte
para direct-only e falham rapidamente quando nao existem provedores conformes.

Catalogos canonicos de opt-out vivem em
`governance/compliance/soranet_opt_outs.json`; o Conselho de Governanca publica
atualizacoes via releases tagueadas. Um exemplo completo de configuracao
(incluindo attestations) esta disponivel em
`docs/examples/sorafs_compliance_policy.json`, e o processo operacional esta
capturado no
[playbook de conformidade GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Ajustes de CLI e SDK

| Flag / Campo | Efeito |
|--------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limita quantos provedores sobrevivem ao filtro do scoreboard. Defina `None` para usar todos os provedores elegiveis. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limita retries por chunk. Exceder o limite gera `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Injeta snapshots de latencia/falha no construtor do scoreboard. Telemetria obsoleta alem de `telemetry_grace_secs` marca provedores como inelegiveis. |
| `--scoreboard-out` | Persiste o scoreboard calculado (provedores elegiveis + inelegiveis) para inspecao pos-run. |
| `--scoreboard-now` | Sobrescreve o timestamp do scoreboard (segundos Unix) para manter capturas de fixtures deterministas. |
| `--deny-provider` / hook de politica de score | Exclui provedores de forma deterministica sem deletar adverts. Util para blacklisting rapido. |
| `--boost-provider=name:delta` | Ajusta os creditos do round-robin ponderado de um provedor mantendo os pesos de governanca intactos. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Rotula metricas emitidas e logs estruturados para que dashboards possam filtrar por geografia ou onda de rollout. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | O padrao agora e `soranet-first` ja que o orquestrador multi-origem e a base. Use `direct-only` ao preparar downgrade ou seguir uma diretriz de conformidade, e reserve `soranet-strict` para pilotos PQ-only; overrides de conformidade continuam sendo o teto rigido. |

SoraNet-first e agora o padrao de envio, e rollbacks devem citar o bloqueador
SNNet relevante. Depois que SNNet-4/5/5a/5b/6a/7/8/12/13 forem graduadas, a
governanca endurecera a postura requerida (rumo a `soranet-strict`); ate la,
apenas overrides motivados por incidentes devem priorizar `direct-only`, e eles
devem ser registrados no log de rollout.

Todos os flags acima aceitam a sintaxe `--` tanto em `sorafs_cli fetch` quanto no
binario `sorafs_fetch` voltado a desenvolvedores. Os SDKs expõem as mesmas opcoes
por meio de builders tipados.

### 1.4 Gestao de cache de guards

A CLI agora integra o seletor de guards da SoraNet para que operadores possam
fixar relays de entrada de forma deterministica antes do rollout completo de
transporte SNNet-5. Tres novos flags controlam o fluxo:

| Flag | Proposito |
|------|-----------|
| `--guard-directory <PATH>` | Aponta para um arquivo JSON que descreve o consenso de relays mais recente (subset abaixo). Passar o directory atualiza o cache de guards antes de executar o fetch. |
| `--guard-cache <PATH>` | Persiste o `GuardSet` codificado em Norito. Execucoes subsequentes reutilizam o cache mesmo sem novo directory. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Overrides opcionais para o numero de guards de entrada a fixar (padrao 3) e a janela de retencao (padrao 30 dias). |
| `--guard-cache-key <HEX>` | Chave opcional de 32 bytes usada para marcar caches de guard com um MAC Blake3 para que o arquivo seja verificado antes da reutilizacao. |

As cargas de directory de guards usam um esquema compacto:

O flag `--guard-directory` agora espera um payload `GuardDirectorySnapshotV2`
codificado em Norito. O snapshot binario contem:

- `version` — versao do esquema (atualmente `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  metadados de consenso que devem corresponder a cada certificado embutido.
- `validation_phase` — gate de politica de certificados (`1` = permitir uma
  assinatura Ed25519, `2` = preferir assinaturas duplas, `3` = exigir assinaturas
  duplas).
- `issuers` — emissores de governanca com `fingerprint`, `ed25519_public` e
  `mldsa65_public`. Os fingerprints sao calculados como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — uma lista de bundles SRCv2 (saida de
  `RelayCertificateBundleV2::to_cbor()`). Cada bundle carrega o descriptor do
  relay, flags de capacidade, politica ML-KEM e assinaturas duplas Ed25519/ML-DSA-65.

A CLI verifica cada bundle contra as chaves de emissor declaradas antes de
mesclar o directory com o cache de guards. Esbocos JSON alternativos nao sao mais
aceitos; snapshots SRCv2 sao obrigatorios.

Invoque a CLI com `--guard-directory` para mesclar o consenso mais recente com o
cache existente. O seletor preserva guards fixados que ainda estao dentro da
janela de retencao e sao elegiveis no directory; novos relays substituem entradas
expiradas. Depois de um fetch bem-sucedido, o cache atualizado e escrito de volta
no caminho fornecido via `--guard-cache`, mantendo sessoes subsequentes
criteriosas. Os SDKs podem reproduzir o mesmo comportamento chamando
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` e
passando o `GuardSet` resultante para `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` permite que o seletor priorize guards com capacidade PQ
quando o rollout SNNet-5 esta em curso. Os toggles de etapa (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) agora rebaixam relays classicos
automaticamente: quando um guard PQ esta disponivel o seletor remove pins
classicos excedentes para que sessoes subsequentes favorecam handshakes hibridos.
Os resumos CLI/SDK expõem a mistura resultante via `anonymity_status`/
`anonymity_reason`, `anonymity_effective_policy`, `anonymity_pq_selected`,
`anonymity_classical_selected`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
e campos complementares de candidatos/deficit/delta de supply, tornando claros
brownouts e fallbacks classicos.

Directories de guards agora podem embutir um bundle SRCv2 completo via
`certificate_base64`. O orquestrador decodifica cada bundle, revalida as
assinaturas Ed25519/ML-DSA e retém o certificado analisado junto ao cache de
guards. Quando um certificado esta presente ele se torna a fonte canonica para
chaves PQ, preferencias de handshake e ponderacao; certificados expirados sao
descartados e o seletor retorna aos campos alternativos do descriptor. Certificados
propagam-se pela gestao do ciclo de vida de circuitos e sao expostos via
`telemetry::sorafs.guard` e `telemetry::sorafs.circuit`, que registram a janela
de validade, suites de handshake e se assinaturas duplas foram observadas para
cada guard.

Use os helpers da CLI para manter snapshots em sincronia com publicadores:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` baixa e verifica o snapshot SRCv2 antes de grava-lo em disco, enquanto
`verify` reproduz o pipeline de validacao para artefatos vindos de outras equipes,
emitindo um resumo JSON que espelha a saida do seletor de guards CLI/SDK.

### 1.5 Gestor de ciclo de vida de circuitos

Quando um relay directory e um cache de guards sao fornecidos, o orquestrador
ativa o gestor de ciclo de vida de circuitos para preconstruir e renovar
circuitos SoraNet antes de cada fetch. A configuracao vive em `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) via dois novos campos:

- `relay_directory`: carrega o snapshot do directory SNNet-3 para que hops
  middle/exit sejam selecionados de forma deterministica.
- `circuit_manager`: configuracao opcional (habilitada por padrao) que controla o
  TTL do circuito.

Norito JSON agora aceita um bloco `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Os SDKs encaminham dados do directory via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), e a CLI o conecta automaticamente sempre que
`--guard-directory` e fornecido (`crates/iroha_cli/src/commands/sorafs.rs:365`).

O gestor renova circuitos sempre que metadados do guard mudam (endpoint, chave
PQ ou timestamp fixado) ou quando o TTL expira. O helper `refresh_circuits`
invocado antes de cada fetch (`crates/sorafs_orchestrator/src/lib.rs:1346`)
emite logs `CircuitEvent` para que operadores possam rastrear decisoes de ciclo
de vida. O soak test `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) demonstra latencia estavel
atraves de tres rotacoes de guards; veja o relatorio em
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC local

O orquestrador pode opcionalmente iniciar um proxy QUIC local para que extensoes
de navegador e adaptadores SDK nao precisem gerenciar certificados ou chaves de
cache de guards. O proxy liga a um endereco loopback, encerra conexoes QUIC e
retorna um manifest Norito descrevendo o certificado e a chave de cache de guard
opcional ao cliente. Eventos de transporte emitidos pelo proxy sao contados via
`sorafs_orchestrator_transport_events_total`.

Habilite o proxy por meio do novo bloco `local_proxy` no JSON do orquestrador:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` controla onde o proxy escuta (use porta `0` para solicitar porta
  efemera).
- `telemetry_label` propaga-se para as metricas para que dashboards distingam
  proxies de sessoes de fetch.
- `guard_cache_key_hex` (opcional) permite que o proxy exponha o mesmo cache de
  guards com chave que CLI/SDKs usam, mantendo extensoes do navegador alinhadas.
- `emit_browser_manifest` alterna se o handshake devolve um manifest que
  extensoes podem armazenar e validar.
- `proxy_mode` seleciona se o proxy faz bridge local (`bridge`) ou apenas emite
  metadados para que SDKs abram circuitos SoraNet por conta propria
  (`metadata-only`). O proxy padrao e `bridge`; use `metadata-only` quando um
  workstation deve expor o manifest sem retransmitir streams.
- `prewarm_circuits`, `max_streams_per_circuit` e `circuit_ttl_hint_secs`
  expõem hints adicionais ao navegador para que possa orcar streams paralelos e
  entender o quanto o proxy reutiliza circuitos.
- `car_bridge` (opcional) aponta para um cache local de arquivos CAR. O campo
  `extension` controla o sufixo anexado quando o alvo de stream omite `*.car`;
  defina `allow_zst = true` para servir payloads `*.car.zst` precomprimidos.
- `kaigi_bridge` (opcional) expõe rotas Kaigi em spool ao proxy. O campo
  `room_policy` anuncia se o bridge opera em modo `public` ou `authenticated`
  para que clientes do navegador preselecionem os labels GAR corretos.
- `sorafs_cli fetch` expõe overrides `--local-proxy-mode=bridge|metadata-only` e
  `--local-proxy-norito-spool=PATH`, permitindo alternar o modo de runtime ou
  apontar para spools alternativos sem modificar a politica JSON.
- `downgrade_remediation` (opcional) configura o hook de downgrade automatico.
  Quando habilitado, o orquestrador observa a telemetria de relays para rajadas
  de downgrade e, apos o `threshold` configurado dentro de `window_secs`, força o
  proxy local para o `target_mode` (padrao `metadata-only`). Quando os downgrades
  cessam, o proxy retorna ao `resume_mode` apos `cooldown_secs`. Use o array
  `modes` para limitar o gatilho a funcoes de relay especificas (padrao relays
  de entrada).

Quando o proxy roda em modo bridge ele serve dois servicos de aplicacao:

- **`norito`** – o alvo de stream do cliente e resolvido relativo a
  `norito_bridge.spool_dir`. Os alvos sao sanitizados (sem traversal, sem caminhos
  absolutos), e quando o arquivo nao tem extensao, o sufixo configurado e aplicado
  antes do payload ser transmitido para o navegador.
- **`car`** – alvos de stream se resolvem dentro de `car_bridge.cache_dir`, herdam
  a extensao padrao configurada e rejeitam payloads comprimidos a menos que
  `allow_zst` esteja habilitado. Bridges bem-sucedidos respondem com `STREAM_ACK_OK`
  antes de transferir os bytes do arquivo para que clientes possam fazer pipeline
  da verificacao.

Em ambos os casos o proxy fornece o HMAC do cache-tag (quando havia uma chave de
cache de guard durante o handshake) e registra codigos de razao de telemetria
`norito_*` / `car_*` para que dashboards diferenciem sucessos, arquivos ausentes
e falhas de sanitizacao rapidamente.

`Orchestrator::local_proxy().await` expõe o handle em execucao para que chamadas
possam ler o PEM do certificado, buscar o manifest do navegador ou solicitar
encerramento gracioso quando a aplicacao finaliza.

Quando habilitado, o proxy agora serve registros **manifest v2**. Alem do
certificado existente e da chave de cache de guard, a v2 adiciona:

- `alpn` (`"sorafs-proxy/1"`) e um array `capabilities` para que clientes
  confirmem o protocolo de stream que devem falar.
- Um `session_id` por handshake e um bloco de sal `cache_tagging` para derivar
  afinidades de guard por sessao e tags HMAC.
- Hints de circuito e selecao de guard (`circuit`, `guard_selection`,
  `route_hints`) para que integracoes do navegador exponham uma UI mais rica antes
  de abrir streams.
- `telemetry_v2` com knobs de amostragem e privacidade para instrumentacao local.
- Cada `STREAM_ACK_OK` inclui `cache_tag_hex`. Clientes espelham o valor no header
  `x-sorafs-cache-tag` ao emitir requisicoes HTTP ou TCP para que selecoes de guard
  em cache permaneçam criptografadas em repouso.

Esses campos fazem parte do schema atual; clientes devem utilizar o conjunto
completo ao negociar streams.

## 2. Semantica de falhas

O orquestrador aplica verificacoes estritas de capacidade e budgets antes que um
unico byte seja transferido. As falhas se enquadram em tres categorias:

1. **Falhas de elegibilidade (pre-flight).** Provedores sem capacidade de range,
   adverts expirados ou telemetria obsoleta sao registrados no artefato do
   scoreboard e omitidos do agendamento. Resumos da CLI preenchem o array
   `ineligible_providers` com razoes para que operadores inspecionem drift de
   governanca sem raspar logs.
2. **Esgotamento em runtime.** Cada provedor rastreia falhas consecutivas. Quando
   `provider_failure_threshold` e atingido, o provedor e marcado como `disabled`
   pelo restante da sessao. Se todos os provedores transicionarem para `disabled`,
   o orquestrador retorna
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Abortos deterministas.** Limites rigidos surgem como erros estruturados:
   - `MultiSourceError::NoCompatibleProviders` — o manifest exige um span de
     chunks ou alinhamento que os provedores restantes nao conseguem honrar.
   - `MultiSourceError::ExhaustedRetries` — o budget de retries por chunk foi
     consumido.
   - `MultiSourceError::ObserverFailed` — observadores downstream (hooks de
     streaming) rejeitaram um chunk verificado.

Cada erro incorpora o indice do chunk problemático e, quando disponivel, a razao
final de falha do provedor. Trate esses erros como bloqueadores de release —
retries com a mesma entrada reproduzirao a falha ate que o advert, a telemetria
ou a saude do provedor subjacente mudem.

### 2.1 Persistencia do scoreboard

Quando `persist_path` e configurado, o orquestrador escreve o scoreboard final
apos cada run. O documento JSON contem:

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (peso normalizado atribuido para este run).
- metadados do `provider` (identificador, endpoints, budget de concorrencia).

Arquive snapshots do scoreboard junto aos artefatos de release para que decisoes
de banimento e rollout permaneçam auditaveis.

## 3. Telemetria e depuracao

### 3.1 Metricas Prometheus

O orquestrador emite as seguintes metricas via `iroha_telemetry`:

| Metrica | Labels | Descricao |
|---------|--------|-----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge de fetches orquestrados em voo. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histograma registrando latencia de fetch de ponta a ponta. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de falhas terminais (retries esgotados, sem provedores, falha de observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Contador de tentativas de retry por provedor. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Contador de falhas de provedor na sessao que levam a desabilitacao. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Contagem de decisoes de politica de anonimato (cumprida vs brownout) agrupadas por estagio de rollout e motivo de fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histograma da participacao de relays PQ no conjunto SoraNet selecionado. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histograma de ratios de oferta de relays PQ no snapshot do scoreboard. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histograma do deficit de politica (gap entre alvo e a participacao PQ real). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histograma da participacao de relays classicos usada em cada sessao. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histograma de contagens de relays classicos selecionados por sessao. |

Integre as metricas em dashboards de staging antes de habilitar knobs de
producao. O layout recomendado espelha o plano de observabilidade SF-6:

1. **Fetches ativos** — alerta se o gauge sobe sem completions correspondentes.
2. **Razao de retries** — avisa quando contadores `retry` excedem baselines
   historicas.
3. **Falhas de provedor** — dispara alertas no pager quando qualquer provedor
   cruza `session_failure > 0` dentro de 15 minutos.

### 3.2 Targets de log estruturados

O orquestrador publica eventos estruturados para targets deterministas:

- `telemetry::sorafs.fetch.lifecycle` — marcadores `start` e `complete` com
  contagem de chunks, retries e duracao total.
- `telemetry::sorafs.fetch.retry` — eventos de retry (`provider`, `reason`,
  `attempts`) para alimentar triage manual.
- `telemetry::sorafs.fetch.provider_failure` — provedores desabilitados devido a
  erros repetidos.
- `telemetry::sorafs.fetch.error` — falhas terminais resumidas com `reason` e
  metadados opcionais do provedor.

Encaminhe esses fluxos para o pipeline de logs Norito existente para que a
resposta a incidentes tenha uma unica fonte de verdade. Eventos de ciclo de vida
exponem a mistura PQ/classica via `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` e seus contadores associados,
tornando simples integrar dashboards sem raspar metricas. Durante rollouts de
GA, fixe o nivel de log em `info` para eventos de ciclo de vida/retry e use
`warn` para falhas terminais.

### 3.3 Resumos JSON

Tanto `sorafs_cli fetch` quanto o SDK Rust retornam um resumo estruturado contendo:

- `provider_reports` com contagens de sucesso/falha e se o provedor foi
  desabilitado.
- `chunk_receipts` detalhando qual provedor atendeu cada chunk.
- arrays `retry_stats` e `ineligible_providers`.

Arquive o arquivo de resumo ao depurar provedores problemáticos — os receipts
mapeiam diretamente para os metadados de log acima.

## 4. Checklist operacional

1. **Preparar configuracao no CI.** Execute `sorafs_fetch` com a configuracao
   alvo, passe `--scoreboard-out` para capturar a visao de elegibilidade e
   compare com o release anterior. Qualquer provedor inelegivel inesperado
   interrompe a promocao.
2. **Validar telemetria.** Garanta que o deploy exporta metricas `sorafs.fetch.*`
   e logs estruturados antes de habilitar fetches multi-origem para usuarios. A
   ausencia de metricas normalmente indica que a fachada do orquestrador nao foi
   invocada.
3. **Documentar overrides.** Ao aplicar `--deny-provider` ou `--boost-provider`
   emergenciais, comite o JSON (ou a invocacao CLI) no changelog. Rollbacks devem
   reverter o override e capturar um novo snapshot do scoreboard.
4. **Reexecutar smoke tests.** Depois de modificar budgets de retry ou limites de
   provedores, refaça o fetch do fixture canonico
   (`fixtures/sorafs_manifest/ci_sample/`) e verifique que os receipts de chunks
   permanecem deterministas.

Seguir os passos acima mantém o comportamento do orquestrador reproduzivel em
rollouts por fase e fornece a telemetria necessaria para resposta a incidentes.

### 4.1 Overrides de politica

Operadores podem fixar o estagio ativo de transporte/anonimato sem editar a
configuracao base definindo `policy_override.transport_policy` e
`policy_override.anonymity_policy` em seu JSON de `orchestrator` (ou fornecendo
`--transport-policy-override=` / `--anonymity-policy-override=` ao
`sorafs_cli fetch`). Quando qualquer override esta presente, o orquestrador pula
o fallback brownout usual: se o nivel PQ solicitado nao puder ser satisfeito, o
fetch falha com `no providers` em vez de degradar silenciosamente. O rollback
para o comportamento padrao e tao simples quanto limpar os campos de override.
