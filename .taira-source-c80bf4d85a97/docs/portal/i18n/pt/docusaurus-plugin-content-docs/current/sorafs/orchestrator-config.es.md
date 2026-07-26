---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orquestrador-config
título: Configuração do orquestrador de SoraFS
sidebar_label: Configuração do orquestrador
description: Configura o orquestrador de busca de múltiplas origens, interpreta falhas e depura a saída de telemetria.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/developer/orchestrator.md`. Mantenha ambas as cópias sincronizadas até que os documentos herdados sejam retirados.
:::

# Guia do orquestrador de busca de origem múltipla

O orquestrador de busca multi-origem de SoraFS impulsa descargas deterministas e
paralelamente ao conjunto de provedores publicados em anúncios respaldados por
a governança. Este guia explica como configurar o orquestrador, que senales de
falo esperar durante os lançamentos e os fluxos de telemetria expõem indicadores
de saúde.

## 1. Resumo de configuração

O orquestrador combina três fontes de configuração:

| Fonte | Proposta | Notas |
|--------|-----------|-------|
| `OrchestratorConfig.scoreboard` | Normaliza os pesos dos provedores, valida o frescor da telemetria e persiste o scoreboard JSON usado para auditórios. | Respaldado por `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Aplicar limites de tempo de execução (pressupostos de reintenção, limites de concorrência, alternâncias de verificação). | É mapeado para `FetchOptions` e `crates/sorafs_car::multi_fetch`. |
| Parâmetros de CLI/SDK | Limita o número de pares, ajuda regiões de telemetria e expõe políticas de negação/aumento. | `sorafs_cli fetch` expor esses flags diretamente; o SDK é transferido via `OrchestratorConfig`. |

Os helpers JSON em `crates/sorafs_orchestrator::bindings` serializaram
configuração completa em Norito JSON, para que haja um portátil entre ligações de
SDK e automação.

### 1.1 Configuração JSON de exemplo

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

Persista o arquivo por meio da compilação habitual de `iroha_config` (`defaults/`,
usuário, atual) para que os despliegues deterministas herdem os limites mismos
entre nós. Para um perfil de fallback diretamente alinhado com o rollout
SNNet-5a, consulte `docs/examples/sorafs_direct_mode_policy.json` e o guia
complementaria em `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Anulações de cumprimento

SNNet-9 integra o cumprimento dirigido pelo governo no orquestrador. Un
novo objeto `compliance` na configuração Norito JSON captura os carve-outs
que acionou o pipeline de busca no modo somente direto:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` declara os códigos ISO-3166 alfa-2 onde opera esta
  instância do orquestrador. Os códigos são normalizados e maiúsculos durante o
  análise.
- `jurisdiction_opt_outs` reflete o registro de governo. Quando alguma coisa
  jurisdição do operador aparece na lista, o solicitante aplica-se
  `transport_policy=direct-only` e emite a razão de fallback
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` lista resumos de manifesto (CIDs cegados, codificados em
  hexadecimal mayusculas). As cargas coincidentes também forçam o planejamento
  direct-only e expõe o substituto `compliance_blinded_cid_opt_out` no
  telemetria.
- `audit_contacts` registra o URI que o governo espera que os operadores
  publique em seus playbooks GAR.
- `attestations` captura os pacotes de cumprimento firmados que respaldam la
  política. Cada entrada define um `jurisdiction` opcional (código ISO-3166
  alpha-2), um `document_uri`, o `digest_hex` canônico de 64 caracteres, o
  timestamp de emissão `issued_at_ms` e um `expires_at_ms` opcional. Estos
  artefatos alimentam a lista de verificação dos auditórios do orquestrador para que o
  as ferramentas de governança podem vincular as anulações à documentação
  firmada.

Proporciona o bloqueio de conformidade mediante a apilação habitual de
configuração para que os operadores recebam anulações deterministas. El
aplicativo orquestrador compliance _despues_ das dicas de modo de gravação: mesmo si
um SDK solicitado `upload-pq-only`, exclusões por jurisdição ou manifestação
siguen forzando transporte direct-only e fallan rapido quando não existir
provadores aptos.

Os catálogos canônicos de opt-out vivem em
`governance/compliance/soranet_opt_outs.json`; el Conselho de Governo Publico
atualizações mediante lançamentos etiquetados. Um exemplo completo de
configuração (incluindo atestados) está disponível em
`docs/examples/sorafs_compliance_policy.json`, e o processo operacional é
captura no el
[manual de cumprimento GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Ajustes de CLI e SDK| Bandeira / Campo | Efeito |
|--------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limite de quantos provedores sobreviveram no filtro do placar. Ponlo en `None` para usar todos os fornecedores elegíveis. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limite as reintenções por pedaço. Superar o limite do gênero `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Injete instantâneos de latência/queda no construtor do placar. A telemetria obsoleta, mas em `telemetry_grace_secs` é marcada como fornecedora como inelegível. |
| `--scoreboard-out` | Persistir o cálculo do placar (provedores elegíveis + inelegíveis) para inspeção posterior. |
| `--scoreboard-now` | Substitua o carimbo de data/hora do placar (segundos Unix) para que as capturas de jogos sigam os deterministas. |
| `--deny-provider` / gancho de política de pontuação | Excluir deterministicamente provedores de planejamento sem borrar anúncios. Util para listas negras de resposta rápida. |
| `--boost-provider=name:delta` | Ajusta os créditos de round-robin ponderados para um provedor mantendo intactos os pesos de governo. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Etiquetas métricas emitidas e logs estruturados para que os painéis possam ser filtrados por geografia e fase de implementação. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Por defeito é `soranet-first` agora que o orquestrador multi-origem está na base. Usa `direct-only` para preparar um downgrade ou seguir uma diretriz de cumprimento, e reserva `soranet-strict` para pilotos somente PQ; as anulações de conformidade continuarão sendo a tecnologia difícil. |

SoraNet-first é agora o valor por defeito, e as reversões devem citar o
bloquearr SNNet correspondente. Tras graduado SNNet-4/5/5a/5b/6a/7/8/12/13,
a governança suporta a postura exigida (para `soranet-strict`); até agora
Então, apenas as anulações motivadas por incidentes devem ser priorizadas
`direct-only`, e deve ser registrado no log de implementação.

Todas as bandeiras anteriores aceitam sintaxe estilo `--` tanto em
`sorafs_cli fetch` como no binário `sorafs_fetch` orientado a
desarrolladores. O SDK expõe as diversas opções por meio de construtores informados.

### 1.4 Gerenciamento de cache de guardas

A CLI agora integra o seletor de proteção do SoraNet para os operadores
pode-se estabelecer relés de entrada de forma determinista antes da implementação completa
SNNet-5 de transporte. Três novas bandeiras controlam o fluxo:| Bandeira | Proposta |
|------|-----------|
| `--guard-directory <PATH>` | Basta acessar um arquivo JSON que descreve o consenso de relés mais recente (mostra um subconjunto abaixo). Passe o diretório para atualizar o cache de guardas antes de executar a busca. |
| `--guard-cache <PATH>` | Persiste o `GuardSet` codificado em Norito. As execuções posteriores reutilizam o cache, mesmo quando um novo diretório não é fornecido. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Substitui opcionais para o número de guardas de entrada a fijar (por defeito 3) e a janela de retenção (por defeito 30 dias). |
| `--guard-cache-key <HEX>` | Clave opcional de 32 bytes usada para etiquetar caches de proteção com um MAC Blake3 para que o arquivo possa ser verificado antes de ser reutilizado. |

As cargas do diretório de guardas usam um esquema compacto:

A bandeira `--guard-directory` agora espera uma carga útil `GuardDirectorySnapshotV2`
codificado em Norito. O binário do snapshot contém:

- `version` — versão do esquema (na verdade `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  metadados de consenso que devem coincidir com cada certificado embebido.
- `validation_phase` — computador de política de certificados (`1` = permitir
  uma única firma Ed25519, `2` = preferir firmas dobles, `3` = requerir firmas
  duplos).
- `issuers` — emissores de governo com `fingerprint`, `ed25519_public` e
  `mldsa65_public`. As impressões digitais são calculadas como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — uma lista de pacotes SRCv2 (saída de
  `RelayCertificateBundleV2::to_cbor()`). Cada pacote inclui o descritor do
  relé, bandeiras de capacidade, política ML-KEM e firmas duplas Ed25519/ML-DSA-65.

A CLI verifica cada pacote contra as chaves do emissor declaradas antes de
fundir o diretório com o cache de guardas. Os esquemas JSON herdados você
não se aceite; são necessários snapshots SRCv2.

Invoque a CLI com `--guard-directory` para fundir o consenso mais recente com
o cache existente. O seletor mantém as proteções fixadas que você está dentro
la ventana de retenção y son elegibles en el directorio; os relés novos
reemplazan entradas expiradas. Após uma busca exitosa, o cache atualizado é
escreva de novo na rota indicada por `--guard-cache`, mantendo sessões
subseqüentes deterministas. O SDK pode reproduzir o mesmo comportamento
chamada `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
e passar o `GuardSet` resultando em `SorafsGatewayFetchOptions`.`ml_kem_public_hex` permite que o seletor priorize proteja com capacidade PQ
quando o SNNet-5 é usado. Os alternadores de etapa (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) agora degradan relés automaticamente
clássicos: quando há um guarda PQ disponível el seletor elimina los pines
clássicos sobrantes para que as sessões posteriores favoreçam apertos de mão
híbridos. Os resumos do CLI/SDK expõem a mistura resultante via
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` e os campos complementares de candidatos/déficit/
variação de fornecimento, deixando explícitos os quedas de energia e os clássicos substitutos.

Os diretórios de guardas agora podem incorporar um pacote SRCv2 completo via
`certificate_base64`. El orquestrador decodifica cada pacote, revalida as firmas
Ed25519/ML-DSA e conserva o certificado analisado junto ao cache de guardas.
Quando um certificado presente for convertido na fonte canônica para
chaves PQ, preferências de aperto de mão e ponderação; os certificados expirados
é descartado e o seletor retorna aos campos herdados do descritor. Los
certificados são propagados na gestão do ciclo de vida de circuitos e se
expor via `telemetry::sorafs.guard` e `telemetry::sorafs.circuit`, que registra
la ventana de validez, las suites de handshake e si se observaron firmas dobles
para cada guarda.

Use os ajudantes da CLI para manter os snapshots sincronizados com os
publicadores:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` baixe e verifique o snapshot SRCv2 antes de escrevê-lo no disco,
enquanto `verify` repete o pipeline de validação para artefatos obtidos
para outros equipamentos, emitindo um currículo JSON que reflete a saída do seletor
de guardas em CLI/SDK.

### 1.5 Gestor de ciclo de vida de circuitos

Quando for fornecido tanto o diretório de relés quanto o cache de guardas, o
orquestrador ativa o gestor de ciclo de vida de circuitos para pré-construir e
atualize os circuitos do SoraNet antes de cada busca. A configuração viva em
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) via dos campos
novos:

- `relay_directory`: leva o instantâneo do diretório SNNet-3 para os saltos
  middle/exit pode selecionar a forma determinista.
- `circuit_manager`: configuração opcional (habilitada por defeito) que
  controla o TTL do circuito.

Norito JSON agora aceita um bloco `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

O SDK reenfian os dados do diretório via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), e o CLI se conecta automaticamente sempre
que se suministra `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).O gestor renova circuitos quando muda os metadados da guarda (endpoint,
chave PQ ou timestamp fixado) ou quando vencer o TTL. El ajudante `refresh_circuits`
invocado antes de cada busca (`crates/sorafs_orchestrator/src/lib.rs:1346`)
emite logs de `CircuitEvent` para que os operadores rastreiem decisões do
ciclo de vida. El teste de imersão
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) demonstra latência estável a
traves de três rotações de guardas; consulte o relatório em
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC local

O sequestrador pode iniciar opcionalmente um proxy QUIC local para que
extensões do navegador e adaptadores SDK não precisam ser gerenciados
certificados ou chaves de cache de guardas. O proxy é colocado em uma direção
loopback, termina conexões QUIC e desenvolve um manifesto Norito que descreve o
certificado e a chave de cache de proteção opcional ao cliente. Os eventos de
transportes emitidos por el proxy se contabilizan via
`sorafs_orchestrator_transport_events_total`.

Habilite o proxy através do novo bloco `local_proxy` no JSON do
orquestrador:

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
```- `bind_addr` controla onde ouve o proxy (usa porto `0` para solicitar
  un porto efímero).
- `telemetry_label` se propaga às métricas para que os painéis possam
  distinguir proxies de sessões de busca.
- `guard_cache_key_hex` (opcional) permite que o proxy exponha o mesmo cache
  de guardas com chave que usam CLI/SDK, mantendo as extensões alinhadas
  do navegador.
- `emit_browser_manifest` alterna se o handshake apresentar um manifesto que
  extensões podem armazenar e validar.
- `proxy_mode` seleciona se a porta proxy de tráfego local (`bridge`) ou
  apenas emite metadados para que o SDK abra circuitos SoraNet por sua conta
  (`metadata-only`). O proxy por defeito é `bridge`; estabelecimento `metadata-only`
  quando uma estação de trabalho deba expõe o manifesto sem reenviar streams.
- `prewarm_circuits`, `max_streams_per_circuit` e `circuit_ttl_hint_secs`
  expor dicas adicionais ao navegador para que você possa presupuestar streams
  paralelos e entendo que tão agressivamente o proxy reutiliza circuitos.
- `car_bridge` (opcional) para colocar um cache local de arquivos CAR. O campo
  `extension` controla o sufixo agregado quando o objetivo do stream é omitido
  `*.car`; estabelecer `allow_zst = true` para servir cargas úteis `*.car.zst`
  pré-comprimidos diretamente.
- `kaigi_bridge` (opcional) expor rotas Kaigi em spool para proxy. O campo
  `room_policy` anuncia se a ponte opera no modo `public` ou `authenticated`
  para que os clientes do navegador pré-selecionem as etiquetas GAR
  correta.
- A exposição `sorafs_cli fetch` substitui `--local-proxy-mode=bridge|metadata-only`
  e `--local-proxy-norito-spool=PATH`, permitindo alternar o modo de
  executar ou colocar spools alternativos sem modificar a política JSON.
- `downgrade_remediation` (opcional) configura o gancho de downgrade automático.
  Quando o orquestrador está habilitado a observar a telemetria dos relés para
  detectar rafagas de downgrade e, após `threshold` configurado dentro de
  `window_secs`, ativa o proxy local em `target_mode` (por padrão
  `metadata-only`). Uma vez que os downgrades cessam, o proxy volta a
  `resume_mode` após `cooldown_secs`. Use o arreglo `modes` para limitar
  el aciona uma função de relé específica (por defeito relés de entrada).

Quando o proxy é executado no modo bridge sirve dos serviços de aplicação:

- **`norito`** – o objetivo de fluxo do cliente é resolvido em relação a
  `norito_bridge.spool_dir`. Os objetivos são higienizados (sin traversal ni rutas
  absolutas) e, quando o arquivo não tem extensão, aplica-se o sufijo
  configurado antes de transmitir a carga útil literalmente para o navegador.
- **`car`** – os objetivos de stream são resolvidos dentro de
  `car_bridge.cache_dir`, herdou a extensão por defeito configurado e
  rechazan payloads comprimidos menos que `allow_zst` está ativado. Los
  pontes exitosos respondem com `STREAM_ACK_OK` antes de transferir los bytes
  do arquivo para que os clientes possam canalizar a verificação.Em ambos os casos, o proxy entrega o HMAC de cache-tag (quando existia uma chave
cache de guarda durante o handshake) e registrar códigos de razão de telemetria
`norito_*` / `car_*` para que os painéis sejam diferentes saídas, arquivos
faltantes e falhas de higienização de um vistazo.

`Orchestrator::local_proxy().await` expõe o identificador na execução para que
Chamadores podem ler o PEM do certificado, obter o manifesto do
navegador ou solicitar um computador desligado quando o aplicativo for finalizado.

Quando estiver habilitado, o proxy agora será enviado aos registros **manifest v2**. Ademas do
certificado existente e a chave de cache de guarda, v2 agrega:

- `alpn` (`"sorafs-proxy/1"`) e um arquivo `capabilities` para os clientes
  confirme o protocolo de stream que você deve usar.
- Um `session_id` por handshake e um bloco de sal `cache_tagging` para derivar
  camadas de proteção por sessão e tags HMAC.
- Dicas de circuito e seleção de guarda (`circuit`, `guard_selection`,
  `route_hints`) para que as integrações do navegador exponham uma UI mas
  rica antes de abrir streams.
- `telemetry_v2` com botões de exibição e privacidade para instrumentação local.
- Cada `STREAM_ACK_OK` inclui `cache_tag_hex`. Os clientes refletem o valor em
  o cabeçalho `x-sorafs-cache-tag` para emitir solicitações HTTP ou TCP para que
  seleciona de guarda e cache permanente cifradas em repouso.

Esses campos preservam o formato anterior; clientes antigos podem ignorar
as novas chaves e continue usando o subconjunto v1.

## 2. Semântica de falhas

O orquestrador aplica testes rigorosos de capacidade e pressupostos antes
que foi transferido um único byte. Los fallos caen em três categorias:

1. **Fallos de elegibilidad (pré-voo).** Proveedores sin capacidad de rango,
   anúncios expirados ou telemetria obsoleta quedan registrados no artefato
   do placar e é omitido na planificação. Os currículos da CLI completam
   o arreglo `ineligible_providers` com razões para que os operadores possam
   inspecionar a deriva da governança sem raspar logs.
2. **Agotamiento en tiempo de ejecucion.** Cada provedor registra falhas
   consecutivos. Uma vez que você alcance o `provider_failure_threshold`
   configurado, o provedor é marcado como `disabled` no restante da sessão.
   Se todos os provedores passarem para `disabled`, o orquestrador deve voltar
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Abortos deterministas.** Limites restritos são apresentados como erros
   estruturados:
   - `MultiSourceError::NoCompatibleProviders` — a manifestação requer um tramo
     de pedaços ou alinhamento que os fornecedores restantes não podem cumprir.
   - `MultiSourceError::ExhaustedRetries` — consome o pressuposto de
     reintentos por chunk.
   - `MultiSourceError::ObserverFailed` — os observadores downstream (ganchos de
     streaming) rechazaron um pedaço selecionado.Cada erro inclui o índice do pedaço problemático e, quando está disponível,
la razon final de fallo del proveedor. Trata esses erros como bloqueadores de
release: los reintentos con la misma entrada reproduzindo a falha até que
mude o anúncio, a telemetria ou a saúde do provedor subyacente.

### 2.1 Persistência do placar

Quando se configura `persist_path`, o orquestrador escreve o placar final
tras cada execução. O documento JSON contém:

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (peso normalizado atribuído para esta execução).
- metadados do `provider` (identificador, endpoints, pressuposto de
  concorrência).

Arquive os instantâneos do placar junto com os artefatos de lançamento para que
as decisões de lista negra e implementação seguem sendo auditáveis.

## 3. Telemetria e depuração

### 3.1 Métricas Prometheus

O orquestrador emite as seguintes métricas via `iroha_telemetry`:

| Métrica | Etiquetas | Descrição |
|--------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Medidor de buscas orquestradas em vôo. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histograma que registra a latência de busca de extremo a extremo. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de falhas terminais (reintentos agotados, sin proveedores, falha do observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Contador de intenções de reintenção por provedor. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Contador de falhas no nível de sessão que leva a desativar provedores. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Conteúdo de decisões políticas anônimas (cumplida vs brownout) agrupadas por etapa de implementação e motivo de fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histograma da cota de relés PQ dentro do conjunto SoraNet selecionado. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histograma de taxas de oferta de relés PQ no instantâneo do placar. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histograma do déficit político (brecha entre o objetivo e a cota PQ real). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histograma da cota de relés clássicos usados ​​em cada sessão. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histograma de conteúdo de relés clássicos selecionados por sessão. |

Integre as métricas nos painéis de teste antes de ativar os botões de
produção. O layout recomendado reflete o plano de observação SF-6:

1. **Busca ativos** — alerta se o medidor sube sem conclusões equivalentes.
2. **Ratio de reintentos** — avisa quando os contadores `retry` superan
   linhas de base históricas.
3. **Fallos de provedor** — dispara alertas ao pager quando qualquer provedor
   supera `session_failure > 0` em 15 minutos.### 3.2 Alvos de logs estruturados

O orquestrador publica eventos estruturados em alvos deterministas:

- `telemetry::sorafs.fetch.lifecycle` — marcas de ciclo de vida `start` e
  `complete` com conteúdo de pedaços, reintenções e duração total.
- `telemetry::sorafs.fetch.retry` — eventos de reintenção (`provider`, `reason`,
  `attempts`) para alimentação e triagem manual.
- `telemetry::sorafs.fetch.provider_failure` — provedores deshabilitados por
  erros repetidos.
- `telemetry::sorafs.fetch.error` — falha nos terminais resumidos com `reason` e
  metadados opcionais do provedor.

Envie esses fluxos para o pipeline de logs Norito existente para que a resposta a
incidentes têm uma única fonte de verdade. Os eventos do ciclo de vida
expor a mistura PQ/clássica via `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` e seus contadores associados,
o que facilita a instalação de painéis sem raspar métricas. Durante os lançamentos do GA,
fixe o nível de logs em `info` para eventos de ciclo de vida/reintenção e uso
`warn` para falhas nos terminais.

### 3.3 Currículos JSON

Tanto `sorafs_cli fetch` quanto o SDK de Rust desenvolveram um currículo estruturado
que contém:

- `provider_reports` com conteúdo de saída/fracaso e se um provedor foi
  desativado.
- `chunk_receipts` detalhando o que prova ou resolve cada pedaço.
- arranjos `retry_stats` e `ineligible_providers`.

Arquivar o arquivo de currículo para limpar fornecedores problemáticos: os recibos
mapeie diretamente aos metadados dos logs anteriores.

## 4. Checklist operativo

1. **Prepare a configuração em CI.** Execute `sorafs_fetch` com
   configuração objetivo, pasa `--scoreboard-out` para capturar a vista de
   elegibilidade e comparação com a liberação anterior. Cualquier provado ou inelegível
   inesperado detiene la promocion.
2. **Validar telemetria.** Certifique-se de que o despliegue exporta métricas
   `sorafs.fetch.*` e logs estruturados antes de habilitar buscas de origem múltipla
   para usuários. A ausência de métricas geralmente indica que a fachada do
   orquestrador não foi invocado.
3. **Substituições documentais.** Ao aplicar ajustes de emergência `--deny-provider`
   ou `--boost-provider`, confirme o JSON (ou a chamada CLI) em seu changelog.
   As reversões devem reverter a substituição e capturar um novo instantâneo do
   placar.
4. **Repita os testes de fumaça.** Altere os requisitos de reintenção ou limites
   de provedores, volte a fazer buscar del fixture canonico
   (`fixtures/sorafs_manifest/ci_sample/`) e verifica se os recibos de
   pedaços sigan siendo deterministas.

Seguindo os passos anteriores, mantenha o comportamento do orquestrador
reproduzível em implementações por fases e fornece a telemetria necessária para
a resposta a incidentes.

### 4.1 Anulações políticasOs operadores podem estabelecer a etapa ativa de transporte/anonimato sem editar
a configuração base estabelecendo `policy_override.transport_policy` e
`policy_override.anonymity_policy` em seu JSON de `orchestrator` (ou ministrando
`--transport-policy-override=` / `--anonymity-policy-override=` a
`sorafs_cli fetch`). Quando qualquer substituição das substituições está presente
orquestrador omite o brownout alternativo habitual: se o nível PQ solicitado não se
pode ser satisfatório, a busca falha com `no providers` em vez de degradar em
silêncio. Voltar ao comportamento por defeito é tão simples como limpá-los
campos de substituição.