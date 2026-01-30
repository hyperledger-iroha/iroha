---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-storage.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: node-storage
title: Design de storage do nodo SoraFS
sidebar_label: Design de storage do nodo
description: Arquitetura de storage, quotas e hooks de ciclo de vida para nodes Torii que hospedam dados SoraFS.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/sorafs_node_storage.md`. Mantenha ambas as copias sincronizadas.
:::

## Design de storage do nodo SoraFS (Draft)

Esta nota refina como um nodo Iroha (Torii) pode optar pela camada de
availability de dados do SoraFS e dedicar um pedaco do disco local para
armazenar e servir chunks. Ela complementa a especificacao de discovery
`sorafs_node_client_protocol.md` e o trabalho de fixtures SF-1b ao detalhar a
arquitetura do lado do storage, controles de recursos e plumbing de
configuracao que devem aterrissar no nodo e nos caminhos do gateway.
Os drills operacionais ficam no
[Runbook de operacoes do nodo](./node-operations).

### Objetivos

- Permitir que qualquer validador ou processo auxiliar do Iroha exponha disco
  ocioso como provedor SoraFS sem afetar as responsabilidades do ledger.
- Manter o modulo de storage deterministico e guiado por Norito: manifests,
  planos de chunk, raizes Proof-of-Retrievability (PoR) e adverts de provedor sao
  a fonte de verdade.
- Impor quotas definidas pelo operador para que um nodo nao esgote seus
  recursos ao aceitar demasiadas requisicoes de pin ou fetch.
- Expor saude/telemetria (amostragem PoR, latencia de fetch de chunk, pressao de
  disco) de volta para governanca e clientes.

### Arquitetura de alto nivel

```
+--------------------------------------------------------------------+
|                         Iroha/Torii Node                           |
|                                                                    |
|  +----------+      +----------------------+                        |
|  | Torii APIs|<---->|    SoraFS Gateway   |<---------------+       |
|  +----------+      |  (Norito endpoints)  |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |     Pin Registry     |<---- manifests |       |
|                    |     (State / DB)     |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |     Chunk Storage    |<---- chunk plans|       |
|                    |      (ChunkStore)    |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |    Disk Quota/IO     |--pin/serve----->| Fetch |
|                    |      Scheduler       |                | Clients|
|                    +----------------------+                |       |
|                                                                    |
+--------------------------------------------------------------------+
```

Modulos chave:

- **Gateway**: expoe endpoints HTTP Norito para propostas de pin, requisicoes de
  fetch de chunks, amostragem PoR e telemetria. Valida payloads Norito e
  encaminha requisicoes para o chunk store. Reutiliza o stack HTTP do Torii para
  evitar um novo daemon.
- **Pin Registry**: estado de pin de manifest rastreado em `iroha_data_model::sorafs`
  e `iroha_core`. Quando um manifest e aceito o registry registra o digest do
  manifest, digest do plano de chunk, raiz PoR e flags de capacidade do provedor.
- **Chunk Storage**: implementacao `ChunkStore` em disco que ingere manifests
  assinados, materializa planos de chunk usando `ChunkProfile::DEFAULT`, e
  persiste chunks em layout deterministico. Cada chunk e associado a um
  fingerprint de conteudo e metadados PoR para que a amostragem possa revalidar
  sem reler o arquivo inteiro.
- **Quota/Scheduler**: impoe limites configurados pelo operador (bytes max de
  disco, pins pendentes max, fetches paralelos max, TTL de chunk) e coordena IO
  para que as tarefas do ledger nao fiquem sem recursos. O scheduler tambem e
  responsavel por servir provas PoR e requisicoes de amostragem com CPU limitada.

### Configuracao

Adicione uma nova secao em `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # tag opcional legivel
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: toggle de participacao. Quando false o gateway retorna 503 para
  endpoints de storage e o nodo nao anuncia em discovery.
- `data_dir`: diretorio raiz para dados de chunk, arvores PoR e telemetria de
  fetch. Default `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: limite rigido para dados de chunk pinados. Uma tarefa de
  fundo rejeita novos pins quando o limite e atingido.
- `max_parallel_fetches`: limite de concorrencia imposto pelo scheduler para
  balancear banda/IO de disco contra a carga do validador.
- `max_pins`: maximo de pins de manifest que o nodo aceita antes de aplicar
  eviccao/back pressure.
- `por_sample_interval_secs`: cadencia para jobs automaticos de amostragem PoR.
  Cada job amostra `N` folhas (configuravel por manifest) e emite eventos de
  telemetria. A governanca pode escalar `N` de forma deterministica definindo a
  chave de metadata `profile.sample_multiplier` (inteiro `1-4`). O valor pode
  ser um numero/string unico ou um objeto com overrides por perfil, por exemplo
  `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estrutura usada pelo gerador de advert para preencher campos
  `ProviderAdvertV1` (stake pointer, hints de QoS, topics). Se omitido o nodo
  usa defaults do registry de governanca.

Plumbing de configuracao:

- `[sorafs.storage]` e definido em `iroha_config` como `SorafsStorage` e e
  carregado do arquivo de config do nodo.
- `iroha_core` e `iroha_torii` passam a configuracao de storage para o builder
  do gateway e o chunk store no startup.
- Overrides de dev/test existem (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), mas
  deploys de producao devem confiar no arquivo de config.

### Utilitarios de CLI

Enquanto a superficie HTTP do Torii ainda esta sendo ligada, o crate
`sorafs_node` envia uma CLI leve para que operadores possam automatizar drills de
ingestao/exportacao contra o backend persistente. [crates/sorafs_node/src/bin/sorafs-node.rs:1]

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` espera um manifest `.to` codificado em Norito mais os bytes de payload
  correspondentes. Ele reconstroi o plano de chunk a partir do perfil de
  chunking do manifest, impoe paridade de digest, persiste arquivos de chunk e
  opcionalmente emite um blob JSON `chunk_fetch_specs` para que tooling downstream
  valide o layout.
- `export` aceita um ID de manifest e grava o manifest/payload armazenado em
  disco (com plan JSON opcional) para que fixtures continuem reproduziveis.

Ambos os comandos imprimem um resumo Norito JSON em stdout, facilitando o uso em
scripts. A CLI e coberta por um teste de integracao para garantir que manifests e
payloads fazem round-trip corretamente antes das APIs Torii entrarem. [crates/sorafs_node/tests/cli.rs:1]

> Paridade HTTP
>
> O gateway Torii agora expoe helpers read-only apoiados pelo mesmo
> `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` - retorna o manifest
>   Norito armazenado (base64) junto com digest/metadata. [crates/iroha_torii/src/sorafs/api.rs:1207]
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` - retorna o plano de chunk
>   deterministico JSON (`chunk_fetch_specs`) para tooling downstream. [crates/iroha_torii/src/sorafs/api.rs:1259]
>
> Esses endpoints espelham a saida do CLI para que pipelines possam trocar
> scripts locais por probes HTTP sem mudar parsers. [crates/iroha_torii/src/sorafs/api.rs:1207] [crates/iroha_torii/src/sorafs/api.rs:1259]

### Ciclo de vida do nodo

1. **Startup**:
   - Se storage estiver habilitado o nodo inicializa o chunk store com o
     diretorio e capacidade configurados. Isso inclui verificar ou criar a base
     de dados de manifest PoR e fazer replay de manifests pinados para aquecer
     caches.
   - Registrar as rotas do gateway SoraFS (endpoints Norito JSON POST/GET para pin,
     fetch, amostragem PoR, telemetria).
   - Iniciar o worker de amostragem PoR e o monitor de quotas.
2. **Discovery / Adverts**:
   - Gerar documentos `ProviderAdvertV1` usando capacidade/saude atual, assinar
     com a chave aprovada pelo conselho e publicar via discovery. Use a lista
3. **Fluxo de pin**:
   - O gateway recebe um manifest assinado (incluindo plano de chunk, raiz PoR,
     assinaturas do conselho). Valida a lista de aliases (`sorafs.sf1@1.0.0`
     requerido) e garante que o plano de chunk corresponde a metadata do manifest.
   - Verifica quotas. Se capacidade/limites de pin excederem, responde com erro
     de politica (Norito estruturado).
   - Stream de dados de chunk para o `ChunkStore`, verificando digests durante
     a ingestao. Atualiza arvores PoR e armazena metadata do manifest no registry.
4. **Fluxo de fetch**:
   - Serve requisicoes de range de chunk a partir do disco. O scheduler impoe
     `max_parallel_fetches` e retorna `429` quando saturado.
   - Emite telemetria estruturada (Norito JSON) com latencia, bytes servidos e
     contadores de erro para monitoramento downstream.
5. **Amostragem PoR**:
   - O worker seleciona manifests proporcionalmente ao peso (por exemplo, bytes
     armazenados) e roda amostragem deterministica usando a arvore PoR do chunk store.
   - Persiste resultados para auditorias de governanca e inclui resumos em adverts
     de provedor / endpoints de telemetria.
6. **Eviccao / cumprimento de quotas**:
   - Quando a capacidade e atingida o nodo rejeita novos pins por padrao. Opcionalmente,
     operadores podem configurar politicas de eviccao (ex: TTL, LRU) quando o modelo
     de governanca estiver definido; por ora o design assume quotas estritas e
     operacoes de unpin iniciadas pelo operador.

### Declaracao de capacidade e integracao de scheduling

- Torii agora repassa updates de `CapacityDeclarationRecord` de `/v1/sorafs/capacity/declare`
  para o `CapacityManager` embutido, de modo que cada nodo constroi uma visao em memoria
  de suas alocacoes comprometidas de chunker e lane. O manager expoe snapshots read-only
  para telemetria (`GET /v1/sorafs/capacity/state`) e impoe reservas por perfil ou lane
  antes de novas ordens serem aceitas. [crates/sorafs_node/src/capacity.rs:1] [crates/sorafs_node/src/lib.rs:60]
- O endpoint `/v1/sorafs/capacity/schedule` aceita payloads `ReplicationOrderV1`
  emitidos pela governanca. Quando a ordem mira o provedor local o manager verifica
  scheduling duplicado, valida capacidade de chunker/lane, reserva o slice e retorna
  um `ReplicationPlan` descrevendo capacidade restante para que ferramentas de
  orquestracao possam seguir com a ingestao. Ordens para outros provedores sao
  reconhecidas com resposta `ignored` para facilitar workflows multi-operador. [crates/iroha_torii/src/routing.rs:4845]
- Hooks de conclusao (por exemplo, disparados apos ingestao bem sucedida) chamam
  `POST /v1/sorafs/capacity/complete` para liberar reservas via
  `CapacityManager::complete_order`. A resposta inclui um snapshot
  `ReplicationRelease` (totais restantes, residuais de chunker/lane) para que tooling
  de orquestracao possa enfileirar a proxima ordem sem polling. Trabalho futuro
  vai conectar isso ao pipeline do chunk store assim que a logica de ingestao chegar.
  [crates/iroha_torii/src/routing.rs:4885] [crates/sorafs_node/src/capacity.rs:90]
- O `TelemetryAccumulator` embutido pode ser mutado via
  `NodeHandle::update_telemetry`, permitindo que workers de background registrem
  amostras de PoR/uptime e eventualmente derivem payloads canonicos
  `CapacityTelemetryV1` sem tocar nos internos do scheduler. [crates/sorafs_node/src/lib.rs:142] [crates/sorafs_node/src/telemetry.rs:1]

### Integracoes e trabalho futuro

- **Governanca**: estender `sorafs_pin_registry_tracker.md` com telemetria de
  storage (taxa de sucesso PoR, utilizacao de disco). Politicas de admissao podem
  exigir capacidade minima ou taxa minima de sucesso PoR antes de aceitar adverts.
- **SDKs de cliente**: expor a nova configuracao de storage (limites de disco, alias)
  para que ferramentas de gestao possam bootstrapar nodes programaticamente.
- **Telemetria**: integrar com o stack de metricas existente (Prometheus /
  OpenTelemetry) para que metricas de storage aparecam em dashboards de observabilidade.
- **Seguranca**: rodar o modulo de storage em um pool dedicado de tarefas async com
  back-pressure e considerar sandboxing de leituras de chunk via io_uring ou pools
  tokio limitados para evitar que clientes maliciosos esgotem recursos.

Este design mantem o modulo de storage opcional e deterministico enquanto fornece
os knobs necessarios para operadores participarem da camada de availability de dados
SoraFS. Implementa-lo exigira mudancas em `iroha_config`, `iroha_core`, `iroha_torii`
e no gateway Norito, alem do tooling de advert de provedor.
