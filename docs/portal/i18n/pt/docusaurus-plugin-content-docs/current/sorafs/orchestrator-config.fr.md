---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orquestrador-config
título: Configuração do orquestrador SoraFS
sidebar_label: Configuração do orquestrador
description: Configura o orquestrador de busca de múltiplas fontes, interpreta os cheques e desativa a telemetria.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/developer/orchestrator.md`. Garanta que as duas cópias sejam sincronizadas até que a documentação herdada seja retirada.
:::

# Guia do orquestrador de busca multi-fonte

O orquestrador de busca multi-fonte de SoraFS piloto de download
paralelos e determinados a partir do conjunto de fornecedores publicados no
anúncios soutenus par la gouvernance. Este guia configurador de comentários explícitos
l'orchestrateur, quels signaux d'échec compareçam durante os lançamentos e outros
fluxo de télémétrie expondo indicadores de saúde.

## 1. Vista do conjunto de configuração

L'orchestrateur fusionne três fontes de configuração:

| Fonte | Objetivo | Notas |
|-------|----------|-------|
| `OrchestratorConfig.scoreboard` | Normalize o peso dos fornecedores, valide a taxa de transferência e persista o scoreboard JSON usado para auditorias. | Acessado em `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Applique des limites d’execution (orçamentos de nova tentativa, borne de concurrence, bascules de verification). | Mapeado para `FetchOptions` em `crates/sorafs_car::multi_fetch`. |
| Parâmetros CLI/SDK | Limitar o nome dos pares, anexar as regiões de telecomunicações e expor as políticas de negação/reforço. | `sorafs_cli fetch` expõe o direcionamento dos sinalizadores ces; O SDK é propagado via `OrchestratorConfig`. |

Os auxiliares JSON em `crates/sorafs_orchestrator::bindings` serializam-no
configuração completa em Norito JSON, o rendante portátil entre vinculações SDK e
automatização.

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

Persista o arquivo por meio do compilador habitual `iroha_config` (`defaults/`, user,
atual) para que as implantações determinem os mesmos limites
les noœuds. Para um perfil de resposta alinhado somente diretamente no lançamento SNNet-5a,
consulte `docs/examples/sorafs_direct_mode_policy.json` e as indicações
associados em `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Substituições de conformidade

SNNet-9 integra a conformidade piloto do governo do orquestrador.
Um novo objeto `compliance` na configuração Norito arquivos de captura JSON
carve-outs que forçam o pipeline de busca no modo somente direto:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` declara os códigos ISO‑3166 alfa‑2 ou abrir este
  instance de l'orchestrateur. Os códigos são normalizados em letras maiúsculas quando você
  análise.
- `jurisdiction_opt_outs` reflete o registro de governo. Lorsqu'une
  jurisdição opérateur apparaît dans la liste, l'orchestrateur impõe
  `transport_policy=direct-only` e a razão do fallback
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` lista os resumos do manifesto (CIDs mascarados, codificados em
  hexadécimal maiúsculo). Les payloads correspondentes forcent aussi une
  planejamento somente direto e exposição do substituto
  `compliance_blinded_cid_opt_out` na televisão.
- `audit_contacts` registra o URI que o governo atende aos operadores
  em seus manuais GAR.
- `attestations` captura os pacotes de conformidade assinados que justificam
  política. Cada entrada definida é um opcional `jurisdiction` (código ISO‑3166
  alpha‑2), um `document_uri`, o `digest_hex` canônico de 64 caracteres, o
  carimbo de data e hora de emissão `issued_at_ms` e uma opção `expires_at_ms`. Ces
  artefatos que alimentam a lista de verificação de auditoria do orquestrador para que eles
  ferramentas de governo podem confiar nas substituições de documentos assinados.

Forneça o bloco de conformidade por meio do compilador habitual de configuração para
que os operadores recebam substituições determinadas. L'orchestrateur
aplique a conformidade _après_ com as dicas de modo de gravação: mesmo se um SDK exigir
`upload-pq-only`, les opt-outs de jurisdiction ou de manifest basculent toujours
ver o transporte somente direto e ecoar rapidamente para o fornecedor
conform n’existe.

Os catálogos canônicos de exclusão de residentes em
`governance/compliance/soranet_opt_outs.json`; Conselho de Governo Público
as mises do dia através dos lançamentos marcados. Um exemplo de configuração completa
(incluindo os atestados) está disponível em
`docs/examples/sorafs_compliance_policy.json`, e o processo operacional é
capturado no
[manual de conformidade GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Parâmetros CLI e SDK| Bandeira/Campeão | Efeito |
|-------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limite o número de fornecedores que passam pelo filtro do placar. Use `None` para usar todos os fornecedores qualificados. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Plafonne as novas tentativas por pedaço. Ultrapasse o limite de desaceleração `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Injeta instantâneos de latência/chec no construtor do placar. Um telefone enviado a partir de `telemetry_grace_secs` torna os fornecedores inelegíveis. |
| `--scoreboard-out` | Persista o placar calculado (quadros elegíveis + inelegíveis) para inspeção pós-corrida. |
| `--scoreboard-now` | Sobrecarga o carimbo de data/hora do placar (segundos Unix) para monitorar as capturas de jogos determinadas. |
| `--deny-provider` / gancho de política de pontuação | Exclua os fornecedores de maneira determinada sem apagar os anúncios. Útil para uma lista negra com resposta rápida. |
| `--boost-provider=name:delta` | Ajustar os créditos do round-robin seria um fornecedor sem tocar nos pesos de governo. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Etiquete métricas e registros estruturados para que os painéis possam ser girados por geografia ou vaga de implementação. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Por padrão `soranet-first`, o orquestrador multifonte é a base. Use `direct-only` para um downgrade ou uma diretiva de conformidade e reserve `soranet-strict` para pilotos somente PQ; as substituições de conformidade permanecem no teto. |

SoraNet-first está desordenando o padrão liberado e as reversões devem ser citadas
correspondente bloquant da SNNet. Após a formatura de SNNet-4/5/5a/5b/6a/7/8/12/13,
o governo durcira a postura necessária (versão `soranet-strict`); d'ici là, les
seusls substitui os motivos do incidente doivent privilégier `direct-only`, et ils
são enviados no log de implementação.

Todas as bandeiras ci-dessus aceitam a sintaxe `--` em `sorafs_cli fetch` et le
binário orientado para desenvolvedores `sorafs_fetch`. O SDK expõe as mesmas opções
via des construtores digitados.

### 1.4 Gerenciamento do cache de guardas

O cabo CLI desmonta o seletor de proteção SoraNet para permitir
operadores de saída controlam os relés de entrada determinados antes de
implementação concluída do transporte SNNet-5. Três novas bandeiras controlam esse fluxo:| Bandeira | Objetivo |
|------|----------|
| `--guard-directory <PATH>` | Aponte para um arquivo JSON que descreve o consenso dos relés mais recentes (sous-ensemble ci-dessous). Passe o diretório para recuperar o cache dos guardas antes de executar a busca. |
| `--guard-cache <PATH>` | Persista o `GuardSet` codificado em Norito. As execuções seguintes utilizam o cache mesmo sem o novo diretório. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Substitui opções para o nome dos guardas de entrada para o espigão (por padrão 3) e a janela de retenção (por padrão 30 dias). |
| `--guard-cache-key <HEX>` | Use uma opção de 32 octetos para marcar caches de segurança com um MAC Blake3 para verificar o arquivo antes de reutilizá-lo. |

As cargas úteis do diretório de guardas usam um esquema compacto:

A bandeira `--guard-directory` atende a uma carga útil `GuardDirectorySnapshotV2`
codificado em Norito. O conteúdo binário do instantâneo:

- `version` — versão do esquema (atual `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  metadonées de consenso devant correspondente a cada certificado integrado.
- `validation_phase` — portão de política de certificados (`1` = autorizar um
  assinatura única Ed25519, `2` = prefere assinaturas duplas, `3` = exigir
  des duplica assinaturas).
- `issuers` — administradores de governo com `fingerprint`, `ed25519_public` et
  `mldsa65_public`. As impressões digitais são calculadas como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — uma lista de pacotes SRCv2 (sortie
  `RelayCertificateBundleV2::to_cbor()`). Cada pacote inclui o descritor do
  relé, bandeiras de capacidade, política ML-KEM e assinaturas duplas
  Ed25519/ML-DSA-65.

A CLI verifica cada pacote com as chaves de registro declaradas antes de
fusionar o diretório com o cache de guardas. Les esquisses JSON heritées ne
são mais aceitos; Os snapshots SRCv2 são necessários.

Ligue para CLI com `--guard-directory` para fundir o consenso mais
recente com o cache existente. O seletor conserva os guardas épinglés encore
válidos na janela de retenção e elegíveis no diretório; os
novos relés substituem entradas expiradas. Após uma tentativa de busca, o cache
meu dia foi escrito no caminho de volta via `--guard-cache`, guarde-os
sessões suivantes déterministes. O SDK reproduz o mesmo comportamento em
recorrente `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
e um injetor `GuardSet` resultante em `SorafsGatewayFetchOptions`.`ml_kem_public_hex` permite ao selecionador priorizar os guardas capazes PQ
enquanto o lançamento do SNNet-5. Os interruptores de fita (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) degrada automaticamente
relés clássicos: quando um guarda PQ estiver disponível, o seletor suprime os
pins classiques en trop para que as sessões seguintes favoreçam
apertos de mão híbridos. Os currículos CLI/SDK expõem o mix resultante via
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` e os campeões associados de candidatos/déficit/delta de
fornecimento, rendant explicita les brownouts et fallbacks clássicos.

Os diretórios de guardas podem ser desordenados para embarcar um pacote SRCv2 completo via
`certificate_base64`. L'orchestrateur decodifica cada pacote, revalide-os
assinaturas Ed25519/ML-DSA e conserve o certificado analisado nos locais do cache
de guardas. Quando um certificado está presente, ele deve a fonte canônica para
as chaves PQ, as preferências de aperto de mão e ponderação; os certificados
expirados são cartões e o seletor revient aux champs herités du descripteur.
Os certificados são propagados na gestão do ciclo de vida dos circuitos e
são expostos via `telemetry::sorafs.guard` e `telemetry::sorafs.circuit`, aqui
consignando a janela de validação, as suítes de aperto de mão e a observação ou
assinaturas não duplas para cada guarda.

Use a CLI auxiliar para manter os snapshots alinhados com os editores:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` baixe e verifique o instantâneo SRCv2 antes de gravar no disco,
então `verify` rejoue o pipeline de validação para os artefatos em questão
de outras equipes, emite um currículo JSON que reflete a seleção do selecionador
de guardas CLI/SDK.

### 1.5 Gerenciamento do ciclo de vida dos circuitos

Quando o diretório de relés e o cache de guardas são fornecidos, o orquestrador
ativa o gerenciamento do ciclo de vida dos circuitos para pré-construir et
renovando circuitos SoraNet antes de cada busca. A configuração foi encontrada
sous `OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) via deux
novos campeões:

- `relay_directory`: transporta o instantâneo do diretório SNNet-3 para esses arquivos
  sauts middle/exit são selecionados de maneira determinada.
- `circuit_manager`: opções de configuração (ativadas por padrão) controlando o
  TTL dos circuitos.

Norito JSON aceita o bloco `circuit_manager` :

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

O SDK transmite os dados do diretório via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), e a CLI conecta o cabo automaticamente
`--guard-directory` é fornecido (`crates/iroha_cli/src/commands/sorafs.rs:365`).O gerente renova os circuitos quando as metas de guarda são alteradas
(endpoint, clé PQ ou timestamp de pinagem) ou quando o TTL expirar. O ajudante
`refresh_circuits` chamado antes de cada busca (`crates/sorafs_orchestrator/src/lib.rs:1346`)
foi criado um registro `CircuitEvent` para que os operadores possam rastrear suas decisões
liées au cycle de vie. Teste de imersão
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) demonstra uma latência estável
sur trois rotações de guardas; veja o relacionamento associado em
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC local

O orquestrador pode lançar uma opção local QUIC para os
O navegador de extensões e os adaptadores SDK não permitem gerenciar certificados
ni les clés du cache de guards. O proxy se encontra em um endereço de loopback, termina
conexões QUIC e reenvio de um manifesto Norito que descreva o certificado e o
clé de cache de guarda optionnelle au client. Os eventos de transporte sãois par
o proxy é computado via `sorafs_orchestrator_transport_events_total`.

Ative o proxy por meio do novo bloco `local_proxy` no JSON do orquestrador:

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
```- `bind_addr` controla o endereço de registro do proxy (utilize a porta `0` para
  exigir um porto éfêmero).
- `telemetry_label` é projetado com métricas para que os painéis sejam diferenciados
  os proxies das sessões de busca.
- `guard_cache_key_hex` (opcional) permite que o proxy exponha o mesmo cache de
  protege o CLI/SDK, protegendo as extensões de navegação alinhadas.
- `emit_browser_manifest` bascula o retorno de um manifesto das extensões
  pode ser armazenado e validado.
- `proxy_mode` escolha se o proxy retransmite o local de tráfego (`bridge`) ou
  mas não há metadonées para que o SDK abra eux-mêmes de circuitos
  SoraNet (`metadata-only`). O proxy é por padrão `bridge` ; escolha
  `metadata-only` quando um poste doit expõe o manifesto sem retransmitir o fluxo.
- `prewarm_circuits`, `max_streams_per_circuit` e `circuit_ttl_hint_secs`
  expor dicas complementares ao navegador para otimizar o fluxo
  paralelamente e compreenda o nível de reutilização de circuitos.
- `car_bridge` (opcional) aponta para um cache local de arquivos CAR. O campeão
  `extension` controla o sufixo adicionado quando o código é adicionado a `*.car` ; definição
  `allow_zst = true` para servir diretamente cargas úteis `*.car.zst` pré-comprimidas.
- `kaigi_bridge` (opcional) expõe as rotas Kaigi armazenadas no proxy. O campeão
  `room_policy` anuncia se a ponte funciona no modo `public` ou
  `authenticated` para que os clientes naveguem pré-selecionando os rótulos
  GAR apropriado.
- `sorafs_cli fetch` expõe as substituições `--local-proxy-mode=bridge|metadata-only`
  e `--local-proxy-norito-spool=PATH`, permite bascular o modo de execução
  ou o ponteiro para os carretéis alternativos sem modificar o JSON político.
- `downgrade_remediation` (opcional) configura o gancho de downgrade automático.
  Quando estiver ativo, o orquestrador vigia a transmissão dos relés para
  detecte rafales de downgrade e, depois de passar do `threshold` em la
  janela `window_secs`, força o proxy local versão `target_mode` (por padrão
  `metadata-only`). Uma vez que os downgrades foram interrompidos, o proxy revient au
  `resume_mode` depois de `cooldown_secs`. Utilize a tabela `modes` para limitar
  desativa as funções de relé específicas (por padrão, os relés de entrada).

Quando o proxy gira em modo bridge, existem dois aplicativos de serviços:- **`norito`** – o código de fluxo do cliente é resolvido por relacionamento com
  `norito_bridge.spool_dir`. Les cibles sont sanitizées (pas de traversal, pas de
  chemins absolus), e quando um arquivo não for estendido, o sufixo configurado
  é um aplicativo antes de transmitir a carga útil do navegador.
- **`car`** – os fluxos de fluxo são resolvidos em `car_bridge.cache_dir`, heritent
  da extensão configurada por padrão e rejeita as cargas compactadas seguras
  si `allow_zst` está ativo. As pontes levantadas são respondidas com `STREAM_ACK_OK`
  antes de transferir os octetos do arquivo para que os clientes possam
  pipeline de verificação.

Nos dois casos, o proxy fornece o HMAC do cache-tag (quando uma chave de cache
était présente lors du handshake) e registre os códigos de razão de télémétrie
`norito_*` / `car_*` para que os painéis se destaquem de um golpe de estado
sucesso, os arquivos manquants e os échecs de sanitização.

`Orchestrator::local_proxy().await` exponha o identificador durante o processo para que eles
os recorrentes podem ler o PEM do certificado, recuperar o manifesto do navegador
ou exigir uma parada graciosa ao fechar o aplicativo.

Quando o proxy estiver ativo, os registros serão desordenados **manifest v2**.
Além do certificado existente e da chave de cache de guarda, a v2 adicionou:

- `alpn` (`"sorafs-proxy/1"`) e um quadro `capabilities` para os clientes
  confirma o protocolo de fluxo a ser utilizado.
- Um `session_id` por aperto de mão e um bloco de salagem `cache_tagging` para derivar
  afinidades de proteção por sessão e tags HMAC.
- Dicas de circuito e seleção de proteção (`circuit`, `guard_selection`,
  `route_hints`) para que as integrações de navegação exponham uma UI mais rica
  avant l'ouverture des flux.
- `telemetry_v2` com botões de encantamento e confidencialidade para
  local de instrumentação.
- Chaque `STREAM_ACK_OK` incluindo `cache_tag_hex`. Os clientes refletem o valor
  no texto `x-sorafs-cache-tag` para solicitações HTTP ou TCP para os
  seleções de guarda no cache restantes chiffrées au repos.

Estes campeões são adicionados ao manifesto v2; os clientes devem consumir
Explicite as chaves que suportam e ignore o resto.

## 2. Sémantique des échecs

O orquestrador aplica verificações rigorosas de capacidades e orçamentos
antes de transferir o menor octeto. Les échecs tombent dans trois categorias:1. **Échecs d’éligibilité (pré-vol).** Les fournisseurs sans capacité de plage,
   aux adverts expirados ou à télémétrie perimée sont consignés dans l’artefact
   du placar et omis de la planification. Os currículos CLI remplissent le
   tabela `ineligible_providers` com as razões para os operadores
   inspeciona os desvios de governo sem raspar os registros.
2. **Épuisement à l’execution.** Chaque fournisseur suit les échecs consécutifs.
   Uma vez `provider_failure_threshold` foi confirmado, o fornecedor está marcado
   `disabled` para o resto da sessão. Si todos os fornecedores deviam
   `disabled`, l'orchestrateur renvoie
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Arrêts déterministes.** Les limites dures remontent sous forme d’erreurs
   estruturas:
   - `MultiSourceError::NoCompatibleProviders` — o manifesto requer um intervalo de
     pedaços ou um alinhamento que os fornecedores restantes não podem honrar.
   - `MultiSourceError::ExhaustedRetries` — o orçamento de novas tentativas por pedaço antes
     consomé.
   - `MultiSourceError::ObserverFailed` — os observadores downstream (ganchos de
     streaming) não rejeitou um pedaço verificado.

Cada erro ao embarcar no índice do pedaço fautif e, quando disponível, a razão
finale d'échec du fournisseur. Traitez ces erreurs como des lockurs de release
— as tentativas com a mesma entrada reproduzem o cheque tanto quanto o anúncio, o
a telemetria ou a saúde do fornecedor subjacente não mudou.

### 2.1 Persistência do placar

Quando `persist_path` está configurado, o orquestrador escreve o placar final
depois de cada corrida. O conteúdo JSON do documento:

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (poids normalizados atribuídos para esta execução).
- metadonnées du `provider` (identificador, endpoints, orçamento de concorrência).

Arquive os instantâneos do placar com os artefatos de lançamento para que eles
escolha de lista negra e implementação restante auditável.

## 3. Telemetria e depuração

### 3.1 Métricas Prometheus

O orquestrador apresentou as seguintes métricas via `iroha_telemetry`:| Métrica | Etiquetas | Descrição |
|--------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Medir as buscas orquestradas no vol. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histograma registra a latência de busca de luta em luta. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Compteur des échecs terminaux (retries épuisés, aucun fournisseur, échec observateur). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Compteur des tentatives de retry par fournisseur. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Compteur des échecs de fournisseur au niveau session significa desativação. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | As decisões políticas anônimas (manutenção vs indisponibilidade) são agrupadas por fase de implementação e razão de retorno. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histograma da parte dos relés PQ no conjunto SoraNet selecionado. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histograma das taxas de oferta dos relés PQ no instantâneo do placar. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histograma do déficit político (o mapa entre o objetivo e a parte PQ real). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histograma da parte dos relés clássicos utilizados em cada sessão. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histograma dos computadores de relés clássicos selecionados por sessão. |

Integre essas métricas nos painéis de teste antes de ativar os botões
em produção. A disposição recomendada reflete o plano de observabilidade SF-6:

1. **Busca ativos** — alerta se o medidor estiver sem conclusões correspondentes.
2. **Proporção de tentativas** — evita quando os computadores `retry` passam
   linhas de base históricas.
3. **Échecs fournisseurs** — desativa os alertas do pager quando um fornecedor
   passe `session_failure > 0` em uma janela de 15 minutos.

### 3.2 Cípulas de logs estruturais

O orquestrador publica eventos estruturados em relação aos círculos determinados:

- `telemetry::sorafs.fetch.lifecycle` — marcas `start` e `complete` com
  nome de pedaços, tentativas e duração total.
- `telemetry::sorafs.fetch.retry` — eventos de nova tentativa (`provider`, `reason`,
  `attempts`) para análise de Manuelle.
- `telemetry::sorafs.fetch.provider_failure` — fornecedores desativados para
  erros repetidos.
- `telemetry::sorafs.fetch.error` — verifica currículos terminais com `reason` e
  metadonnées optionnelles du fournisseur.Obtenha esse fluxo para o pipeline de logs Norito existente após a resposta
aux incidentes têm uma fonte única de verdade. Os eventos do ciclo de vida
expor o mix PQ/classique via `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` e seus computadores associados,
isso facilita a fiação dos painéis sem raspar as métricas. Pingente
as implementações GA bloqueiam o nível de registros em `info` para os eventos do ciclo
tente/repetir e use `warn` para erros terminais.

### 3.3 Currículos JSON

`sorafs_cli fetch` e o SDK Rust enviam um currículo estruturado contendo:

- `provider_reports` com os computadores bem-sucedidos/chegados e o estado de desativação.
- `chunk_receipts` detalhe que fornece um pedaço satisfatório.
- os quadros `retry_stats` e `ineligible_providers`.

Arquive o arquivo de currículo para enviar os fornecedores faltosos:
recibos mappent directement aux métadonnées de logs ci-dessus.

## 4. Checklist operacional

1. **Prepare a configuração em CI.** Lancez `sorafs_fetch` com
   cible de configuração, passe `--scoreboard-out` para capturar a visão
   elegibilidade, e faz uma diferença com o lançamento anterior. Todos os fornecedores
   inelegível inattendu bloque la promoção.
2. **Validar a telemetria.** Certifique-se de que a implantação exporte os
   métricas `sorafs.fetch.*` e os logs estruturados antes de ativar as buscas
   multi-fonte para os usuários. Ausência de métricas indicadas
   que a fachada do orquestrador não foi apelada.
3. **Documente as substituições.** Por meio de `--deny-provider` ou `--boost-provider`
   Em primeiro lugar, envie o JSON (ou a CLI de invocação) para o changelog. Les
   rollbacks devem revogar a substituição e capturar um novo instantâneo de
   placar.
4. **Relance os testes de fumaça.** Depois de modificar os orçamentos de nova tentativa ou de
   caps de fournisseurs, rebuscar la fixture canonique
   (`fixtures/sorafs_manifest/ci_sample/`) e verifique se os recibos de pedaços
   restent déterministas.

Siga as etapas ci-dessus garde o comportamento do orquestrador
reproduzível nas implementações em fases e fornece a tecnologia necessária para
la réponse aux incidentes.

### 4.1 Substituições de política

Os operadores podem detectar a fase de transporte/anônimo ativo sem
modificar a configuração básica em definitivo
`policy_override.transport_policy` e `policy_override.anonymity_policy` em
seu JSON `orchestrator` (ou em quatro versões
`--transport-policy-override=` / `--anonymity-policy-override=` à
`sorafs_cli fetch`). Lorsqu'un override est présent, l'orchestrateur saute le
brownout de substituição habitual: se o nível PQ exigido não pode ser satisfatório,
A busca é ouvida com `no providers` em vez de degradar o silêncio. Le
retour au comportement par défaut consist simplement à vider les champs
substituir.