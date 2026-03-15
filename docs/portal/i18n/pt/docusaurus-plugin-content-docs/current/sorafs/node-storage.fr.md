---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-storage.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: armazenamento de nó
título: Concepção do estoque do nó SoraFS
sidebar_label: Concepção do estoque do nó
descrição: Arquitetura de armazenamento, cotas e ganchos de ciclo de vida para os nódulos Torii, relacionados com os dados SoraFS.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/sorafs_node_storage.md`. Gardez les duas cópias sincronizadas jusqu'au retrait da documentação histórica da Esfinge.
:::

## Conception du stockage du nœud SoraFS (Brouillon)

Esta nota precisa comentar um nó Iroha (Torii) pode ser inscrito no sofá
disponibilidade de dados SoraFS e reserva de uma parte do disco local para
estocar e servir os pedaços. Ela completa a especificação de descoberta
`sorafs_node_client_protocol.md` e o trabalho de luminárias SF-1b detalhado
a arquitetura do estoque, os controles de recursos e o planejamento de
configuração que deve chegar nos caminhos de código do nó e do gateway.
Os operadores de brocas são encontrados no
[Runbook d'opérations du nœud](./node-operations).

### Objetivos

- Permite a validação total ou o processo Iroha auxiliar de exposição do disco
  disponível como provedor SoraFS sem afetar as responsabilidades do razão.
- Garder le module de stockage déterminista et piloté par Norito : manifestos,
  planos de pedaço, provas de recuperação (PoR) e anúncios do provedor
  é a fonte da verdade.
- Aplica cotas definidas pelo operador para que você não consiga fazer isso
  possui recursos e aceita solicitações de pin ou fetch.
- Expor la santé/la télémétrie (échantillonnage PoR, latência de busca de pedaço,
  disco de pressão) para o governo e para os clientes.

### Arquitetura de alto nível

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Módulos clés:- **Gateway** : expõe os endpoints HTTP Norito para as propostas de pin,
  os requisitos de busca de pedaços, o encantamento PoR e a telemetria. Ou
  valide as cargas úteis Norito e obtenha os requisitos para o armazenamento de blocos. Reutilizar
  a pilha HTTP Torii existente para evitar um novo daemon.
- **Registro de Pins**: o estado dos pinos de manifesto subsequentes em `iroha_data_model::sorafs`
  e `iroha_core`. Quando um manifesto for aceito, registre-se para registrar o resumo
  du manifest, le digest du plan de chunk, la racine PoR et les flags de capacité du
  provedor.
- **Chunk Storage**: implementação `ChunkStore` no disco que contém os manifestos
  assinado, materialize os planos de pedaço via `ChunkProfile::DEFAULT` e persista
  os pedaços dependem de um layout determinado. Cada pedaço está associado a uma impressão digital
  de conteúdo e de metadonnées PoR afin que a échantillonnage pode revalidar
  sem precisar do arquivo completo.
- **Quota/Scheduler** : impõe limites configurados pelo operador (bytes disque max,
  pinos na atenção máxima, busca paralelos máximos, TTL de pedaço) e coordena o IO para que
  os registros contábeis não foram divulgados. O agendador é também responsável por
  serviço de testes PoR e solicitações de conversão com CPU nascido.

### Configuração

Adicione uma nova seção em `iroha_config` :

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # tag optionnel lisible
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: alterna a participação. Quando falso, le gateway reenvoie 503 para les
  endpoints de armazenamento e o nó não foi anunciado na descoberta.
- `data_dir`: repertório racine de données de chunk, arbres PoR et télémétrie de
  buscar. Padrão `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: limite de duração para pedaços pinés. Une tâche de fond
  rejeite os novos pinos quando o limite for atingido.
- `max_parallel_fetches`: painel de concorrência imposto pelo agendador para
  Equilibre a banda passante/IO do disco com a carga do validador.
- `max_pins`: nome máximo de pinos de manifesto que o nó aceita antes da aplicação
  despejo/contrapressão.
- `por_sample_interval_secs`: cadência de trabalhos de encantamento automático PoR. Trabalho Chaque
  échantillonne `N` folhas (configurável por manifesto) e contém eventos de telefonia.
  A governança pode ser reduzida `N` de forma determinada através da chave de metadonées
  `profile.sample_multiplier` (entrada `1-4`). O valor pode ser um nome/uma cadeia única
  ou um objeto com substituições por perfil, por exemplo `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estrutura utilizada pelo gerador de anúncios para reparar os campos
  `ProviderAdvertV1` (ponteiro de piquetagem, dicas de QoS, tópicos). Si omis, le noœud utiliza les
  padrões do registro de governo.

Plano de configuração:- `[sorafs.storage]` é definido em `iroha_config` como `SorafsStorage` e carregado
  a partir do arquivo de configuração do nó.
- `iroha_core` e `iroha_torii` transmitem a configuração de armazenamento do construtor gateway
  et au chunk store au démarrage.
- Des substitui dev/test existente (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), mais
  As implantações de produção devem ser acessadas no arquivo de configuração.

### Utilitários CLI

Enquanto a superfície HTTP Torii está ainda no caminho do cabo, a caixa
`sorafs_node` embarque uma CLI fina para que os operadores possam escrever scripts
exercícios de ingestão/exportação para o backend persistente.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` atende a um manifesto `.to` codificado como Norito e os bytes de carga útil associados.
  Ele reconstrui o plano de chunk a partir do perfil de chunking do manifesto, impõe
  a parte dos resumos, persiste os arquivos de pedaços e, como opção, um blob
  JSON `chunk_fetch_specs` para que as ferramentas downstream possam validar o layout.
- `export` aceita um ID de manifesto e escreve o manifesto/carga útil armazenado no disco
  (com plano JSON opcional) para que os equipamentos permaneçam reproduzíveis.

Os dois comandos imprimem um currículo Norito JSON em stdout, o que facilita
tubulação nos scripts. A CLI é protegida por um teste de integração para garantir
que manifestos e cargas úteis são corrigidos de ida e volta antes da chegada das APIs Torii.【crates/sorafs_node/tests/cli.rs:1】

> Paridade HTTP
>
> O gateway Torii expõe os ajudantes desormados em uma aula baseada apenas no mesmo
> `NodeHandle`:
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — reenviar o manifesto
> Norito armazenado (base64) com digest/métadonnées.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — reenviar o plano de pedaço
> determine JSON (`chunk_fetch_specs`) para as ferramentas downstream.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Esses endpoints refletem a saída CLI para que os pipelines possam passar
> os scripts localizados nas sondas HTTP sem trocador de analisadores.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Ciclo de vida do nó1. **Descasamento**:
   - Se o estoque estiver ativo, não será possível inicializar o armazenamento de pedaços com o repertório
     e a capacidade configurada. Cela inclui a verificação ou a criação da base
     de données de manifest PoR et le replay des manifests pinés pour chauffer les caches.
   - Registrar as rotas do gateway SoraFS (endpoints Norito JSON POST/GET para pin,
     buscar, échantillonnage PoR, télémétrie).
   - Lance o trabalhador de encantamento PoR e o monitor de cotas.
2. **Descoberta/Anúncios**:
   - Gere documentos `ProviderAdvertV1` com capacidade/santé atual, assinante
     com a chave aprovada pelo conselho e publicada através do canal de descoberta.
     Use a nova lista `profile_aliases` para proteger os cabos canônicos
3. **Fluxo de trabalho de fixação**:
   - O gateway recebeu um manifesto assinado (incluindo plano de pedaço, racine PoR, assinaturas
     conselho). Valide a lista de alias (requisito `sorafs.sf1@1.0.0`) e garanta que
     o plano de pedaço corresponde às metas do manifesto.
   - Verifique as cotas. Se os limites de capacidade/pinos forem ultrapassados, responda
     por um erro de política (estrutura Norito).
   - Transmita os dados de pedaços em `ChunkStore` e verifique os resumos durante a ingestão.
     Mettre a jour les arbres PoR e armazene os metadonnées du manifest no registre.
4. **Fluxo de trabalho de busca**:
   - Atenda aos requisitos de intervalo de pedaços a partir do disco. Le agendador impor
     `max_parallel_fetches` e reenvie `429` em caso de saturação.
   - Crie uma estrutura de rede (Norito JSON) com latência, serviço de bytes e
     Compteurs d'erreurs para monitoramento a jusante.
5. **Échantillonnage PoR** :
   - O trabalhador seleciona os manifestos proporcionais aos pesos (ex. bytes armazenados)
     e execute uma mudança determinada através da árvore PoR du chunk store.
   - Persistir os resultados das auditorias de governo e incluir currículos em
     provedores de anúncios / endpoints de telefonia.
6. **Despejo/aplicação de cotas**:
   - Quando a capacidade for atingida, os novos pinos não serão rejeitados por padrão. Pt
     opção, os operadores podem configurar as políticas de despejo (ex. TTL, LRU)
     um foi o modelo de governo definido; para o instante, o design supõe que des
     quotas restritas e operações de liberação iniciadas pelo operador.

### Declaração de capacidade e integração de agendamento- Torii relaie désormais les mises a jour `CapacityDeclarationRecord` depois de `/v2/sorafs/capacity/declare`
  vers le `CapacityManager` embarcado, de modo que cada um não construiu uma visão na memória de sessões
  alocações chunker/lane engajados. O gerenciador expõe os instantâneos somente leitura para a transmissão
  (`GET /v2/sorafs/capacity/state`) e aplique reservas por perfil ou por via antes de
  novos comandos não foram aceitos.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- O endpoint `/v2/sorafs/capacity/schedule` aceita cargas úteis `ReplicationOrderV1` enviadas pelo governo.
  Ao solicitar o provedor local, o gerente verifica o planejamento em dobro, valida o
  capacidade chunker/lane, reserve a parcela e reenvie um `ReplicationPlan` que descreva a capacidade restante
  para que as ferramentas de orquestração possam permitir a ingestão. Ordens para outros provedores
  foram adquiridos com uma resposta `ignored` para facilitar os fluxos de trabalho de vários operadores.【crates/iroha_torii/src/routing.rs:4845】
- Des hooks de complétion (por exemplo, déclenchés após sucesso de ingestão) appellent
  `POST /v2/sorafs/capacity/complete` para liberar reservas via `CapacityManager::complete_order`.
  A resposta inclui um instantâneo `ReplicationRelease` (todos os restos, resíduos chunker/lane) para que
  as ferramentas de orquestração podem enfileirar o comando seguinte sem votação. Um trabalho futuro confiável
  cela no pipeline de armazenamento de pedaços quando a lógica de ingestão será pronta.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- Le `TelemetryAccumulator` embarcado pode ser mudo via `NodeHandle::update_telemetry`, permitindo aux
  trabalhadores de fundo de registro de échantillons PoR/uptime e de derivação completa de cargas úteis canônicas
  `CapacityTelemetryV1` sem tocar nos internos do agendador.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integrações e trabalho futuro

- **Governança**: endereço `sorafs_pin_registry_tracker.md` com telemetria de estoque
  (taux de succès PoR, disco de utilização). As políticas de admissão podem exigir uma capacidade
  mínimo ou uma taxa de sucesso PoR mínimo antes de aceitar anúncios.
- **Clientes SDKs**: expõe a nova configuração de armazenamento (limites de disco, alias) para isso
  as ferramentas de gerenciamento podem inicializar as tarefas por programa.
- **Télémétrie** : integrado com a pilha de métricas existente (Prometheus /
  OpenTelemetry) para que as métricas de armazenamento sejam avaliadas nos painéis de observação.
- **Segurança**: execute o módulo de armazenamento em um pool de máquinas assíncronas com contrapressão
  e preveja o sandboxing de palestras de pedaços via io_uring ou des pools tokio bornés para empêcher
  clientes mal-intencionados aproveitam os recursos.Este projeto mantém o módulo de armazenamento opcional e determina todos os dados dos operadores
botões necessários para participar do sofá de disponibilidade de dados SoraFS. Implementação do filho
Implica alterações em `iroha_config`, `iroha_core`, `iroha_torii` e no gateway Norito, também
que as ferramentas de anúncio do provedor.