---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-storage.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: armazenamento de nó
título: Projeto de armazenamento do nó SoraFS
sidebar_label: Design de armazenamento do nó
description: Arquitetura de almacenamiento, cuotas e ganchos do ciclo de vida para nós Torii que alojam dados de SoraFS.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/sorafs_node_storage.md`. Mantenha ambas as cópias sincronizadas até que o conjunto de documentação Sphinx herdado seja retirado.
:::

## Projeto de armazenamento do nó SoraFS (Borrador)

Esta nota refina como um nodo Iroha (Torii) pode optar pela capa de
disponibilidade de dados de SoraFS e dediquei uma parte do disco local para
armazenar e servir pedaços. Complementa a especificação de descoberta
`sorafs_node_client_protocol.md` e o trabalho de luminárias SF-1b em detalhes
arquitetura do lado de armazenamento, controles de recursos e plomeria de
configuração que deve ser aterrada no nó e nas rotas do gateway.
As práticas operacionais vividas no
[Runbook de operações de nó](./node-operations).

### Objetivos

- Permitir que qualquer validador ou processo auxiliar de Iroha exponga disco
  ocioso como provedor SoraFS sem afetar as responsabilidades do razão.
- Mantenha o módulo de armazenamento determinado e guiado por Norito:
  manifestos, planos de pedaços, provas de recuperação (PoR) e anúncios de
  O provedor é a fonte de verdade.
- Imponem cotas definidas pelo operador para que um nó não agrade os seus próprios
  recursos para aceitar solicitações demasiadas de pin ou fetch.
- Exponer saúde/telemetria (mostra PoR, latência de busca de pedaços, pressão
  de discoteca) para governo e clientes.

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

Módulos clave:

- **Gateway**: expor endpoints HTTP Norito para propostas de pin, solicitações
  de buscar pedaços, mostre PoR e telemetria. Cargas úteis de validação Norito y
  enruta las solicitudes hacia el chunk store. Reutilizar a pilha HTTP de Torii
  para evitar um novo daemon.
- **Pin Registry**: estado do pin de manifestos rastreado em `iroha_data_model::sorafs`
  e `iroha_core`. Quando um manifesto é aceito, o registro armazena o resumo
  del manifesto, resumo do plano de pedaço, raiz PoR e sinalizadores de capacidade do
  provedor.
- **Chunk Storage**: implementação `ChunkStore` respaldada pelo disco que foi inserido
  manifesta firmados, materializa planos de pedaço usando `ChunkProfile::DEFAULT`
  e persiste pedaços sob um layout determinista. Cada pedaço é associado a um
  impressão digital de conteúdo e metadados PoR para que o museu possa ser revalidar
  sem ler o arquivo completo.
- **Quota/Scheduler**: impõe limites configurados pelo operador (bytes máximos
  de disco, pinheiros pendientes máximos, busca paralelos máximos, TTL de chunk)
  e coordenar IO para que as tarefas do razão não sejam esgotadas sem recursos. El
  agendador também sirve testes PoR e solicitações de exibição com CPU acotada.

### ConfiguraçãoAgregando uma nova seção em `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # etiqueta opcional legible
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: alterna participação. Quando o gateway responde é falso
  503 para endpoints de armazenamento e o nó não é anunciado na descoberta.
- `data_dir`: diretório raiz para dados de chunk, árvores PoR e telemetria de
  buscar. O padrão é `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: limite de dureza para dados de pedaços fixados. Uma tarefa de
  fondo rechaza nuevos pins quando se alcança o limite.
- `max_parallel_fetches`: topo de concorrência imputado pelo agendador para
  equilibrar ancho de banda/IO de disco com a carga do validador.
- `max_pins`: número máximo de pinos de manifesto que aceita o nodo antes de
  aplicar despejo/contrapressão.
- `por_sample_interval_secs`: cadência para trabalhos automáticos de exibição
  PoR. Cada trabalho mostra `N` hojas (configurável por manifesto) e emite
  eventos de telemetria. A governança pode escalar `N` de forma determinista
  estabelecendo a chave de metadados `profile.sample_multiplier` (insira `1-4`).
  O valor pode ser um número/string único ou um objeto com substituições por perfil,
  por exemplo `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estrutura usada pelo gerador de anúncios para completar campos
  `ProviderAdvertV1` (ponteiro de estaca, dicas de QoS, tópicos). Se omitir o
  nodo usa padrões do registro de governo.

Plomeração de configuração:

- `[sorafs.storage]` é definido em `iroha_config` como `SorafsStorage` e é carregado
  do arquivo de configuração do nodo.
- `iroha_core` e `iroha_torii` passam pela configuração de armazenamento para o
  construtor do gateway e o chunk store na inicialização.
- Existem substituições para dev/test (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), mas
  os aplicativos de produção devem ser baseados no arquivo de configuração.

### Utilitários CLI

Enquanto a superfície HTTP de Torii termina de cabo, a caixa
`sorafs_node` inclui uma CLI liviana para que os operadores possam
automatizar exercícios de ingestão/exportação contra o backend persistente.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` espera um manifesto `.to` codificado em Norito e os bytes de carga útil
  correspondentes. Reconstrua o plano de pedaço a partir do perfil de
  chunking del manifesto, impor paridade de resumos, persistir arquivos de chunk,
  e opcionalmente emite um blob JSON `chunk_fetch_specs` para que as ferramentas
  downstream valida o layout.
- `export` aceita um ID de manifesto e escreve o manifesto/carga útil armazenado em
  disco (com plano JSON opcional) para que os fixtures sigam sendo reproduzíveis
  entre ambientes.

Ambos os comandos imprimem um resumo Norito JSON para stdout, facilitando o uso em
roteiros. A CLI está coberta por um teste de integração para garantir que
manifestos e cargas úteis são reconstituídos corretamente antes de serem iniciados
APIs de Torii.【crates/sorafs_node/tests/cli.rs:1】> Paridade HTTP
>
> El gateway Torii agora exponha ajudantes de solo palestra respaldados pelo mesmo
> `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — retorna o manifesto
> Norito armazenado (base64) junto com digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — devolve o plano de pedaço
> determinista JSON (`chunk_fetch_specs`) para ferramentas downstream.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Esses endpoints refletem a saída da CLI para que os pipelines possam
> passar scripts locais e sondar HTTP sem alterar analisadores.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Ciclo de vida do nodo

1. **Arranjo**:
   - Se o armazenamento estiver habilitado, o nó inicializa o chunk store com
     o diretório e a capacidade configurada. Isso inclui verificar o que criar
     base de dados de manifestos PoR e reprodução de manifestos fixados para calendário
     caches.
   - Registrador de rotas do gateway SoraFS (endpoints Norito JSON POST/GET para pin,
     buscar, museu PoR, telemetria).
   - Lanzar o trabalhador de museu PoR e o monitor de cuotas.
2. **Descoberta/Anúncios**:
   - Gerar documentos `ProviderAdvertV1` usando a capacidade/saúde atual,
     firmá-los com a chave aprovada pelo conselho e publicá-los por meio da descoberta.
     Use a nova lista `profile_aliases` para que os identificadores canônicos e
3. **Flujo de pino**:
   - El gateway recebe um manifesto firmado (incluindo plano de pedaço, raiz PoR,
     firmas del consejo). Valide a lista de alias (`sorafs.sf1@1.0.0` obrigatório)
     e certifique-se de que o plano do bloco coincide com os metadados do manifesto.
   - Verifique cotas. Se a capacidade/limites de pinos for ultrapassada, responda com um
     erro de política (Norito estruturado).
   - Streamea dados de chunk hacia `ChunkStore`, verificando resumos durante la
     ingestão. Atualiza árvores PoR e armazena metadados do manifesto no
     registro.
4. **Flujo de busca**:
   - Sirve solicitudes de rango de chunk desde disco. El agendador impone
     `max_parallel_fetches` e retorna `429` quando está saturado.
   - Emite telemetria estruturada (Norito JSON) com latência, bytes servidos e
     conteúdo de erro para monitoramento downstream.
5. **Muestreo PoR**:
   - A seleção do trabalhador se manifesta proporcionalmente ao peso (por exemplo, bytes
     almacenados) e execute o muestreo determinista usando a árvore PoR do chunk store.
   - Persistir resultados para auditorias de governo e incluir currículos em
     anúncios de provedor / endpoints de telemetria.
6. **Expulsão/cumprimento de cotas**:
   - Quando a capacidade do nodo for alcançada, recarregue novos pinos por defeito.
     Opcionalmente, os operadores podem configurar políticas de expulsão
     (por exemplo, TTL, LRU) uma vez que o modelo de governança é conhecido; por
     agora o design assume cotas estritas e operações de desbloqueio iniciadas por
     o operador.### Declaração de capacidade e integração de agendamento

- Torii agora retransmite atualizações de `CapacityDeclarationRecord` desde
  `/v1/sorafs/capacity/declare` para o `CapacityManager` incorporado, de modo que
  cada nó constrói uma vista em memória de suas atribuições comprometidas de
  pedaço e pista. El manager expõe snapshots de solo palestra para telemetria
  (`GET /v1/sorafs/capacity/state`) e aplique reservas por perfil o lane antes de
  aceitar novos pedidos.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- O endpoint `/v1/sorafs/capacity/schedule` aceita cargas úteis `ReplicationOrderV1`
  emitidos por governo. Cuando la orden apunta al proveedor local el manager
  revisão de agendamento duplicado, verificação de capacidade de chunker/lane, reserva la
  franja e desvia um `ReplicationPlan` descrevendo a capacidade restante para
  que as ferramentas de orquestração continuem a ser ingeridas. Las Ordenes para
  outros fornecedores são reconhecidos com uma resposta `ignored` para facilitar fluxos
  multi-operador.【crates/iroha_torii/src/routing.rs:4845】
- Ganchos de completitud (por exemplo, disparados após o sucesso da ingestão)
  ligue para `POST /v1/sorafs/capacity/complete` para liberar reservas via
  `CapacityManager::complete_order`. A resposta inclui um instantâneo
  `ReplicationRelease` (totais restantes, resíduos de chunker/lane) para que
  as ferramentas de orquestração podem incluir a próxima ordem sem votação.
  O trabalho futuro será conectado ao pipeline do chunk store uma vez que la
  lógica de ingestão aterrice.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- El `TelemetryAccumulator` incorporado pode mudar mediante
  `NodeHandle::update_telemetry`, permitindo que trabalhadores no segundo plano
  registrando demonstrações de PoR/uptime e eventualmente payloads canônicos derivados
  `CapacityTelemetryV1` sem tocar nas internas do agendador.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integrações e trabalho futuro

- **Governança**: extensor `sorafs_pin_registry_tracker.md` com telemetria de
  armazenamento (tasa de sucesso PoR, utilização de disco). As políticas de admissão
  pode exigir capacidade mínima ou taxa mínima de sucesso PoR antes de aceitar
  anúncios.
- **SDKs do cliente**: expõe a nova configuração de armazenamento (limites de
  disco, alias) para que ferramentas de gerenciamento possam inicializar nós
  programática.
- **Telemetria**: integrar com a pilha de especificações existentes (Prometheus /
  OpenTelemetry) para que as métricas de armazenamento apareçam nos painéis de
  observabilidade.
- **Segurança**: execute o módulo de armazenamento em um pool dedicado de tarefas
  assíncrono com contrapressão e considere sandboxing de palestras de pedaços via
  io_uring o pools acotados de tokio para evitar que clientes maliciosos acabem
  recursos.Este projeto mantém o módulo de armazenamento opcional e determinista para o
vez que otorga aos operadores os botões necessários para participar na capa
SoraFS de disponibilidade de dados. Implementar isso implicará mudanças em `iroha_config`,
`iroha_core`, `iroha_torii` e o gateway Norito, além das ferramentas de anúncios
de provedor.