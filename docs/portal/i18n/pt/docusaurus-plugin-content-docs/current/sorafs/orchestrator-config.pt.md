---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orquestrador-config
título: Configuração do orquestrador SoraFS
sidebar_label: Configuração do orquestrador
description: Configure o orquestrador de busca multi-origem, interprete falhas e limpe a saída de telemetria.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/developer/orchestrator.md`. Mantenha ambas as cópias sincronizadas até que a documentação alternativa seja retirada.
:::

# Guia do orquestrador de busca multi-origem

O orquestrador de fetch multi-origem do SoraFS conduz downloads deterministas e
paralelos a partir do conjunto de provedores publicados em anúncios respaldados
pela governança. Este guia explica como configurar o orquestrador, quais sinais
de falha esperar durante rollouts e quais fluxos de telemetria expõem
indicadores de saúde.

## 1. Visão geral da configuração

O orquestrador combina três fontes de configuração:

| Fonte | Proposta | Notas |
|-------|-----------|-------|
| `OrchestratorConfig.scoreboard` | Normaliza pesos de provedores, valida o frescor da telemetria e persiste o scoreboard JSON usado para auditorias. | Apoiado por `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Aplica limites de tempo de execução (orçamentos de repetição, limites de concorrência, alternâncias de verificação). | Mapa para `FetchOptions` em `crates/sorafs_car::multi_fetch`. |
| Parâmetros de CLI/SDK | Limita o número de pares, anexa regiões de telemetria e expõe políticas de negação/impulso. | `sorafs_cli fetch` expõe essas bandeiras diretamente; os SDKs são propagados via `OrchestratorConfig`. |

Os helpers JSON em `crates/sorafs_orchestrator::bindings` serializam a
configuração completa em Norito JSON, tornando-a portavel entre vinculações de SDKs
e automação.

### 1.1 Exemplo de configuração JSON

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
usuário, real) para que implantações deterministas herdem os mesmos limites entre
nós. Para um perfil de fallback direct-only alinhado ao rollout SNNet-5a,
consulte `docs/examples/sorafs_direct_mode_policy.json` e uma orientação
correspondente em `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Substituições de conformidade

A SNNet-9 integra conformidade orientada pela governança no orquestrador. Hum
novo objeto `compliance` na configuração Norito JSON captura os carve-outs que
forçar o pipeline de busca no modo direct-only:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` declara os códigos ISO-3166 alpha-2 onde está
  instância do orquestrador de ópera. Os códigos são normalizados para maiusculas
  durante a análise.
- `jurisdiction_opt_outs` espelha o registro de governança. Quando qualquer
  a jurisdição do operador aparece na lista, o orquestrador aplicador
  `transport_policy=direct-only` e emite o motivo de fallback
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` lista resumos de manifesto (CIDs cegados, codificados em
  hex maiusculo). Payloads correspondentes tambem para cam agendamento direct-only
  e expõe o fallback `compliance_blinded_cid_opt_out` na telemetria.
- `audit_contacts` registra as URIs que a governanca espera que os operadores
  publiquem nos playbooks GAR.
- `attestations` captura dos pacotes de conformidade contratados que sustentam a
  política. Cada entrada define um `jurisdiction` opcional (código ISO-3166
  alpha-2), um `document_uri`, o `digest_hex` canônico de 64 caracteres, o
  timestamp de emissão `issued_at_ms` e um `expires_at_ms` opcional. Esses
  artistas alimentam o checklist de auditórios do orquestrador para que as
  ferramentas de governança podem vincular substituições à documentação assinada.

Forneca o bloco de conformidade via o empilhamento usual de configuração para
que os operadores recebem sobrepõem-se aos deterministas. O orquestrador aplicado a
conformidade _depois_ dos hints de write-mode: mesmo que um SDK solicite
`upload-pq-only`, opt-outs de jurisdição ou manifesto ainda forçam o transporte
para direct-only e falha rapidamente quando não existem provedores conformes.

Catalogos canonicos de opt-out vivem em
`governance/compliance/soranet_opt_outs.json`; o Conselho de Governança Pública
atualizacoes via releases tagueadas. Um exemplo completo de configuração
(incluindo atestados) está disponível em
`docs/examples/sorafs_compliance_policy.json`, e o processo operacional está
capturado não
[manual de conformidade GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Ajustes de CLI e SDK| Bandeira / Campo | Efeito |
|--------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limite quantos provedores sobreviveram ao filtro do placar. Defina `None` para usar todos os provedores elegíveis. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limite de novas tentativas por pedaço. Exceder o limite gera `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Injeta snapshots de latência/falha no construtor do scoreboard. Telemetria obsoleta além de `telemetry_grace_secs` marca provedores como inelegíveis. |
| `--scoreboard-out` | Persiste o cálculo do placar (provadores elegíveis + inelegíveis) para inspeção pós-execução. |
| `--scoreboard-now` | Sobrescreve o timestamp do placar (segundos Unix) para manter capturas de jogos deterministas. |
| `--deny-provider` / gancho de política de pontuação | Exclui provedores de forma determinística sem excluir anúncios. Util para lista negra rápida. |
| `--boost-provider=name:delta` | Ajusta os créditos do round-robin ponderado de um provedor mantendo os pesos de governança intactos. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Rotula métricas emitidas e logs estruturados para que os dashboards possam ser filtrados por geografia ou por onda de implementação. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | O padrão agora e `soranet-first` já que o orquestrador multi-origem e a base. Use `direct-only` ao preparar downgrade ou seguir uma diretriz de conformidade, e reserve `soranet-strict` para pilotos somente PQ; overrides de conformidade continuam sendo o teto rígido. |

SoraNet-first e agora o padrão de envio, e rollbacks devem citar o bloqueador
SNNet relevante. Depois que SNNet-4/5/5a/5b/6a/7/8/12/13 foram graduadas, a
governanca suporta a postura exigida (rumo a `soranet-strict`); comi lá,
apenas overrides motivados por incidentes devem priorizar `direct-only`, e eles
devem ser registrados no log de implementação.

Todas as bandeiras acima aceitam a sintaxe `--` tanto em `sorafs_cli fetch` quanto no
binário `sorafs_fetch` voltado para desenvolvedores. Os SDKs expõem as mesmas opções
por meio de construtores tipados.

### 1.4 Gerenciamento de cache de guardas

A CLI agora integra o seletor de guardas da SoraNet para que os operadores possam
corrigir relés de entrada de forma determinística antes da implementação completa de
transportar SNNet-5. Três novas bandeiras controlam o fluxo:

| Bandeira | Proposta |
|------|-----------|
| `--guard-directory <PATH>` | Aponta para um arquivo JSON que descreve o consenso de relés mais recente (subconjunto abaixo). Passar o diretório atualiza o cache de guardas antes de executar o fetch. |
| `--guard-cache <PATH>` | Persiste o `GuardSet` codificado em Norito. As execuções subsequentes reutilizam o cache mesmo sem novo diretório. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Substitui a notificação para o número de guardas de entrada a fixação (padrão 3) e a janela de retenção (padrão 30 dias). |
| `--guard-cache-key <HEX>` | Chave opcional de 32 bytes usada para marcar caches de guarda com um MAC Blake3 para que o arquivo seja verificado antes da reutilização. |

As cargas de diretório de guardas usam um esquema compacto:O sinalizador `--guard-directory` agora espera um payload `GuardDirectorySnapshotV2`
codificado em Norito. O instantâneo binário contém:

- `version` — versão do esquema (atualmente `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  metadados de consenso que correspondem a cada certificado embutido.
- `validation_phase` — portão de política de certificados (`1` = permitir uma
  assinatura Ed25519, `2` = preferir assinaturas duplas, `3` = exigir assinaturas
  duplas).
- `issuers` — emissores de governança com `fingerprint`, `ed25519_public` e
  `mldsa65_public`. As impressões digitais são calculadas como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — uma lista de pacotes SRCv2 (dita de
  `RelayCertificateBundleV2::to_cbor()`). Cada pacote carrega o descritor do
  relé, bandeiras de capacidade, política ML-KEM e assinaturas duplas Ed25519/ML-DSA-65.

A CLI verifica cada pacote contra as chaves do emissor declaradas antes de
mesclar o diretório com o cache de guardas. Esboços JSON alternativos não são mais
aceito; snapshots SRCv2 são obrigatórios.

Invoque a CLI com `--guard-directory` para mesclar o consenso mais recente com o
cache existente. O seletor preserva guardas definidas que ainda estão dentro da
janela de retenção e são elegíveis no diretório; novos relés substituem entradas
expirado. Depois de uma busca bem-sucedida, o cache atualizado e escrito de volta
no caminho fornecido via `--guard-cache`, mantendo as sessões subsequentes
critérios. Os SDKs podem reproduzir o mesmo comportamento esperado
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` e
passando o `GuardSet` resultando para `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` permite que o seletor priorize guardas com capacidade PQ
quando o lançamento do SNNet-5 está em andamento. Os alternadores de etapa (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) agora rebaixam relés clássicos
automaticamente: quando um guard PQ está disponível o seletor remove pins
clássicos excedentes para que sessões subsequentes favoreçam apertos de mão hibridos.
Os resumos CLI/SDK expõem a mistura resultante via `anonymity_status`/
`anonymity_reason`, `anonymity_effective_policy`, `anonymity_pq_selected`,
`anonymity_classical_selected`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
e campos complementares de candidatos/déficit/delta de oferta, tornando claros
brownouts e fallbacks clássicos.

Diretórios de guardas agora podem incorporar um pacote SRCv2 completo via
`certificate_base64`. O orquestrador decodifica cada pacote, revalida as
assinaturas Ed25519/ML-DSA e retém o certificado desenvolvido junto ao cache de
guardas. Quando um certificado é apresentado ele se torna a fonte canônica para
chaves PQ, preferências de handshake e ponderação; certificados expirados são
descartados e o seletor retorna aos campos alternativos do descritor. Certificados
propagam-se pela gestão do ciclo de vida de circuitos e são expostos via
`telemetry::sorafs.guard` e `telemetry::sorafs.circuit`, que registram uma janela
de validade, suítes de handshake e se assinaturas duplas foram observadas para
cada guarda.Use os helpers da CLI para manter snapshots em sincronia com publicadores:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` baixa e verifica o snapshot SRCv2 antes de gravá-lo em disco, enquanto
`verify` reproduzir o pipeline de validação para artistas vindos de outras equipes,
emitindo um resumo JSON que se espelha na saida do seletor de guardas CLI/SDK.

### 1.5 Gestor de ciclo de vida de circuitos

Quando um diretório de relé e um cache de guardas são fornecidos, o orquestrador
ativa o gestor de ciclo de vida de circuitos para pré-construir e renovar
circuitos SoraNet antes de cada busca. A configuração viva em `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) via dois novos campos:

- `relay_directory`: carrega o snapshot do diretório SNNet-3 para que hops
  middle/exit são selecionados de forma determinística.
- `circuit_manager`: configuração opcional (habilitada por padrão) que controla o
  TTL do circuito.

Norito JSON agora aceita um bloco `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Os SDKs encaminham dados do diretório via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), e a CLI o conecta automaticamente sempre que
`--guard-directory` e fornecido (`crates/iroha_cli/src/commands/sorafs.rs:365`).

O gestor renova circuitos sempre que metadados do guarda mudam (endpoint, chave
PQ ou timestamp fixado) ou quando o TTL expira. Ó ajudante `refresh_circuits`
invocado antes de cada busca (`crates/sorafs_orchestrator/src/lib.rs:1346`)
emite logs `CircuitEvent` para que os operadores possam rastrear decisões de ciclo
de vida. Teste de imersão `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) demonstra latência estavel
através de três rotações de guardas; veja o relatorio em
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC local

O orquestrador pode opcionalmente iniciar um proxy QUIC local para que se estenda
de navegador e adaptadores SDK não precisam gerenciar certificados ou chaves de
cache de guardas. O proxy liga a um endereco loopback, encerra conexões QUIC e
retorna um manifesto Norito descrevendo o certificado e a chave de cache de guarda
opcional ao cliente. Eventos de transporte emitidos por procuração são contados via
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
```- `bind_addr` controla onde o proxy escuta (use a porta `0` para solicitar porta
  efêmera).
- `telemetry_label` propaga-se para as métricas para que os dashboards distingam
  proxies de sessões de busca.
- `guard_cache_key_hex` (opcional) permite que o proxy exponha o mesmo cache de
  guards com chave que CLI/SDKs usam, mantendo extensões do navegador homologadas.
- `emit_browser_manifest` alterna se o handshake devolve um manifesto que
  extensoes podem armazenar e validar.
- `proxy_mode` selecione se o proxy faz bridge local (`bridge`) ou apenas emite
  metadados para que SDKs abram circuitos SoraNet por conta própria
  (`metadata-only`). O proxy padrão e `bridge`; use `metadata-only` quando um
  estação de trabalho deve exportar o manifesto sem retransmitir fluxos.
-`prewarm_circuits`, `max_streams_per_circuit` e `circuit_ttl_hint_secs`
  expõe dicas adicionais ao navegador para que possa orcar streams paralelos e
  entender o quanto o proxy reutiliza circuitos.
- `car_bridge` (opcional) aponta para um cache local de arquivos CAR. O campo
  `extension` controla o sufixo anexado quando o alvo do stream omite `*.car`;
  defina `allow_zst = true` para servir cargas úteis `*.car.zst` pré-comprimidos.
- `kaigi_bridge` (opcional) expõe rotas Kaigi em spool ao proxy. O campo
  `room_policy` anuncia se a ponte opera no modo `public` ou `authenticated`
  para que os clientes do navegador pré-selecionem os rótulos GAR corretos.
- `sorafs_cli fetch` expõe substituições `--local-proxy-mode=bridge|metadata-only` e
  `--local-proxy-norito-spool=PATH`, permitindo alternar o modo de execução ou
  Aponte para spools alternativos sem modificar a política JSON.
- `downgrade_remediation` (opcional) configura o gancho de downgrade automático.
  Quando habilitado, o orquestrador observa a telemetria de relés para rajadas
  de downgrade e, após o `threshold` configurado dentro de `window_secs`, forçar o
  proxy local para o `target_mode` (padrão `metadata-only`). Quando os downgrades
  cessam, o proxy retorna ao `resume_mode` após `cooldown_secs`. Use uma matriz
  `modes` para limitar o gatilho a funções de relé específicas (padrão relés
  de entrada).

Quando o proxy roda em modo bridge ele serve dois serviços de aplicação:

- **`norito`** – o alvo de stream do cliente e resolvido relativo a
  `norito_bridge.spool_dir`. Os alvos são sanitizados (sem travessia, sem caminhos
  absolutos), e quando o arquivo não tem extensão, o sufixo ativado e aplicado
  antes que a carga útil seja transmitida para o navegador.
- **`car`** – alvos de stream se resolvem dentro de `car_bridge.cache_dir`, herdam
  a extensao padrão definido e rejeitam payloads comprimidos a menos que
  `allow_zst` está habilitado. Pontes bem-sucedidas respondidas com `STREAM_ACK_OK`
  antes de transferir os bytes do arquivo para que os clientes possam fazer pipeline
  da verificação.Em ambos os casos o proxy fornece o HMAC do cache-tag (quando havia uma chave de
cache de guarda durante o handshake) e registrar códigos de razão de telemetria
`norito_*` / `car_*` para que dashboards diferenciem sucessos, arquivos ausentes
e falhas de sanitização rapidamente.

`Orchestrator::local_proxy().await` expõe o identificador em execução para que chamadas
pode ler o PEM do certificado, buscar o manifesto do navegador ou solicitar
encerramento gracioso quando a aplicação finaliza.

Quando habilitado, o proxy agora serve registros **manifest v2**. Alem do
certificado existente e a chave de cache de guarda, a v2 adicional:

- `alpn` (`"sorafs-proxy/1"`) e um array `capabilities` para que clientes
  confirme o protocolo de stream que deve falar.
- Um `session_id` por handshake e um bloco de sal `cache_tagging` para derivar
  películas de proteção por sessão e tags HMAC.
- Dicas de circuito e seleção de guarda (`circuit`, `guard_selection`,
  `route_hints`) para que integrações do navegador exponham uma UI mais rica antes
  de abrir fluxos.
- `telemetry_v2` com botões de amostragem e privacidade para instrumentação local.
- Cada `STREAM_ACK_OK` inclui `cache_tag_hex`. Clientes espelham o valor no header
  `x-sorafs-cache-tag` ao emitir requisições HTTP ou TCP para que seleções de guarda
  em cache permanecem criptografadas em reserva.

Esses campos fazem parte do esquema atual; os clientes devem usar o conjunto
completo ao negociar streams.

## 2. Semântica de falhas

O orquestrador aplica verificações de capacidade e orçamentos antes que um
único byte seja transferido. As falhas se enquadraram em três categorias:

1. **Falhas de elegibilidade (pré-voo).** Provedores sem capacidade de alcance,
   anúncios expirados ou telemetria obsoleta são registrados no artistas do
   scoreboard e omitidos do agendamento. Resumos da CLI preenchem o array
   `ineligible_providers` com razões para que operadores inspecionem drift de
   governanca sem raspar logs.
2. **Esgotamento em tempo de execução.** Cada provedor rastreia intervalos consecutivos. Quando
   `provider_failure_threshold` e atingido, o provedor e marcado como `disabled`
   pelo restante da sessão. Se todos os provedores transitarem para `disabled`,
   o orquestrador retorna
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Abortos deterministas.** Limites rígidos surgem como erros estruturados:
   - `MultiSourceError::NoCompatibleProviders` — o manifesto exige um span de
     pedaços ou alinhamento que os provedores restantes não proporcionam honrar.
   - `MultiSourceError::ExhaustedRetries` — o orçamento de novas tentativas por pedaço foi
     consumido.
   - `MultiSourceError::ObserverFailed` — observadores downstream (ganchos de
     streaming) rejeitaram um pedaço selecionado.

Cada erro incorpora o índice do pedaço problemático e, quando disponível, a razão
falha final do provedor. Trate esses erros como bloqueios de lançamento —
retries com a mesma entrada reproduzirao a falha até que o anúncio, a telemetria
ou a saúde do provedor subjacente mudem.

### 2.1 Persistência do placarQuando `persist_path` é configurado, o orquestrador escreve o placar final
após cada corrida. O documento JSON contém:

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (peso normalizado atribuído para esta execução).
- metadados do `provider` (identificador, endpoints, orçamento de concorrência).

Arquive snapshots do scoreboard junto aos artistas de lançamento para que decisões
de banimento e rollout permanecem auditáveis.

## 3. Telemetria e depuração

### 3.1 Métricas Prometheus

O orquestrador emite as seguintes métricas via `iroha_telemetry`:

| Métrica | Etiquetas | Descrição |
|--------|--------|-----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Medidor de buscas orquestradas em voo. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histograma registrando latência de busca de ponta a ponta. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de falhas terminais (retries esgotadas, sem provedores, falha de observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Contador de tentativa de repetição por provedor. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Contador de falhas do provedor na sessão que levam à desabilitação. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Contagem de decisões políticas anônimas (cumprida vs brownout) agrupadas por estágio de implementação e motivo de fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histograma de participação de relés PQ no conjunto SoraNet selecionado. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histograma de índices de oferta de relés PQ no snapshot do scoreboard. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histograma do déficit de política (lacuna entre alvo e participação PQ real). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histograma da participação de relés clássicos usados ​​em cada sessão. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histograma de contagens de relés clássicos selecionados por sessão. |

Integre as métricas em dashboards de staging antes de habilitar botões de
produção. O layout recomendado reflete o plano de observabilidade SF-6:

1. **Buscas ativas** — alerta se o medidor sobe sem conclusões correspondentes.
2. **Razão de novas tentativas** — avisa quando os contadores `retry` excedem as linhas de base
   históricos.
3. **Falhas de provedor** — dispara alertas no pager quando qualquer provedor
   cruza `session_failure > 0` em 15 minutos.

### 3.2 Alvos de log estruturados

O orquestrador público de eventos estruturados para alvos deterministas:- `telemetry::sorafs.fetch.lifecycle` — marcadores `start` e `complete` com
  contagem de chunks, retries e duração total.
- `telemetry::sorafs.fetch.retry` — eventos de nova tentativa (`provider`, `reason`,
  `attempts`) para manual de triagem alimentar.
- `telemetry::sorafs.fetch.provider_failure` — provedores desabilitados devido a
  erros se repetem.
- `telemetry::sorafs.fetch.error` — falhas terminais resumidas com `reason` e
  metadados contributivos do provedor.

Encaminhe esses fluxos para o pipeline de logs Norito existente para que um
resposta a incidentes tenha uma única fonte de verdade. Eventos de ciclo de vida
expor a mistura PQ/classica via `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` e seus contadores associados,
tornando simples integrar dashboards sem raspar métricas. Durante os lançamentos de
GA, fixe o nível de log em `info` para eventos do ciclo de vida/repetir e usar
`warn` para falhas nos terminais.

### 3.3 Resumos JSON

Tanto `sorafs_cli fetch` quanto o SDK Rust retornam um resumo estruturado contendo:

- `provider_reports` com contagens de sucesso/falha e se o provedor foi
  desabilitado.
- `chunk_receipts` detalhando qual provedor atendeu cada pedaço.
- matrizes `retry_stats` e `ineligible_providers`.

Arquive o arquivo de resumo ao depurar provedores problemáticos — os recibos
mapeiam diretamente para os metadados do log acima.

## 4. Checklist operacional

1. **Preparar configuração no CI.** Execute `sorafs_fetch` com a configuração
   alvo, passe `--scoreboard-out` para capturar o visto de elegibilidade e
   compare com o lançamento anterior. Qualquer provedor inelegível inesperado
   interrompeu uma promoção.
2. **Validar telemetria.** Garante que o implante de exportação de métricas `sorafs.fetch.*`
   e logs estruturados antes de habilitar buscas multi-origem para usuários. Um
   Ausência de métricas normalmente indica que a fachada do orquestrador não foi
   invocado.
3. **Substituições documentais.** Ao aplicar `--deny-provider` ou `--boost-provider`
   emergenciais, comite o JSON (ou uma chamada CLI) no changelog. Reversões devem
   reverter o override e capturar um novo snapshot do placar.
4. **Reexecutar testes de fumaça.** Depois de modificar orçamentos de repetição ou limites de
   provedores, refaça o fetch do fixture canonico
   (`fixtures/sorafs_manifest/ci_sample/`) e verifique se os recibos de pedaços
   permanecem deterministas.

Seguir os passos acima mantém o comportamento do orquestrador reproduzivel em
rollouts por fase e fornece a telemetria necessária para resposta a incidentes.

### 4.1 Substituições de política

Os operadores podem fixar o estado ativo de transporte/anonimato sem editar a
configuração base definindo `policy_override.transport_policy` e
`policy_override.anonymity_policy` em seu JSON de `orchestrator` (ou fornece
`--transport-policy-override=` / `--anonymity-policy-override=` ao
`sorafs_cli fetch`). Quando qualquer override está presente, o orquestrador pula
o brownout alternativo usual: se o nível PQ solicitado não puder ser considerado, o
fetch falha com `no providers` em vez de degradar silenciosamente. Ó reversão
para o comportamento padrão e tão simples quanto limpar os campos de override.