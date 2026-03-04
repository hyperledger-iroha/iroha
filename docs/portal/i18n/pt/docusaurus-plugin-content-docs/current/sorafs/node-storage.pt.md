---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-storage.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: armazenamento de nó
título: Design de armazenamento do nodo SoraFS
sidebar_label: Design de armazenamento do nodo
description: Arquitetura de armazenamento, cotas e ganchos de ciclo de vida para nós Torii que hospedam dados SoraFS.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/sorafs_node_storage.md`. Mantenha ambas as cópias sincronizadas.
:::

## Design de armazenamento do nodo SoraFS (Draft)

Esta nota refina como um nodo Iroha (Torii) pode optar pela camada de
disponibilidade de dados do SoraFS e dedicar um pedaco do disco local para
armazenar e servir pedaços. Ela complementa a especificação de descoberta
`sorafs_node_client_protocol.md` e o trabalho de fixtures SF-1b ao detalhar a
arquitetura do lado do armazenamento, controles de recursos e encanamento de
configuração que deve ser aterrada no nodo e nos caminhos do gateway.
Os drills operacionais ficam no
[Runbook de operações do nodo](./node-operations).

### Objetivos

- Permitir que qualquer validador ou processador auxiliar do Iroha exponha disco
  ocioso como provedor SoraFS sem afetar as responsabilidades do ledger.
- Manter o módulo de armazenamento determinístico e orientado por Norito: manifestos,
  planos de chunk, raízes Proof-of-Retrievability (PoR) e anúncios de provedor são
  uma fonte de verdade.
- Importar cotas definidas pelo operador para que um nodo não esgote seus
  recursos para aceitar requisições demasiadas de pin ou fetch.
- Expor saúde/telemetria (amostragem PoR, latência de busca de chunk, pressão de
  disco) de volta para governança e clientes.

### Arquitetura de alto nível

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

Módulos chave:

- **Gateway**: expoe endpoints HTTP Norito para propostas de pin, requisitos de
  fetch de chunks, amostragem PoR e telemetria. Valida cargas úteis Norito e
  encaminha requisições para o chunk store. Reutiliza a pilha HTTP do Torii para
  evitar um novo daemon.
- **Pin Registry**: estado do pin do manifesto rastreado em `iroha_data_model::sorafs`
  e `iroha_core`. Quando um manifesto e aceito o registro registra o resumo do
  manifest, digest do plano de chunk, raiz PoR e flags de capacidade do provedor.
- **Chunk Storage**: implementação `ChunkStore` no disco que gera manifestos
  contratos, materializa planos de chunk usando `ChunkProfile::DEFAULT`, e
  persistir pedaços em layout determinístico. Cada pedaço e associado a um
  impressão digital de conteúdo e metadados PoR para que uma amostra possa ser revalidada
  sem ler o arquivo inteiro.
- **Quota/Scheduler**: impõe limites configurados pelo operador (bytes máx. de
  disco, pinos pendentes max, busca paralelos max, TTL de chunk) e coordenada IO
  para que as tarefas do ledger não fiquem sem recursos. O agendador também e
  responsavel por servir provas PoR e requisições de amostragem com CPU limitada.

### Configuração

Adicione uma nova seção em `iroha_config`:

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
```- `enabled`: alternar participação. Quando false o gateway retorna 503 para
  endpoints de armazenamento e o nodo não anuncia em descoberta.
- `data_dir`: diretorio raiz para dados de chunk, árvores PoR e telemetria de
  buscar. Padrão `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: limite rígido para dados de pedaços pinados. Uma de tarefa
  fundo rejeita novos pinos quando o limite e atingido.
- `max_parallel_fetches`: limite de concorrência imposto pelo agendador para
  balancear banda/IO de disco contra a carga do validador.
- `max_pins`: máximo de pinos de manifesto que o nodo aceita antes de aplicar
  evicção/contrapressão.
- `por_sample_interval_secs`: cadência para trabalhos automáticos de amostragem PoR.
  Cada amostra de trabalho `N` folhas (configurável por manifesto) e emitir eventos de
  telemetria. A governança pode escalar `N` de forma determinística definindo a
  chave de metadados `profile.sample_multiplier` (inteiro `1-4`). O valor pode
  ser um número/string único ou um objeto com overrides por perfil, por exemplo
  `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estrutura usada pelo gerador de anúncio para preenchimento de campos
  `ProviderAdvertV1` (ponteiro de estaca, dicas de QoS, tópicos). Se omitiu o nodo
  padrões dos EUA do registro de governança.

Encanamento de configuração:

- `[sorafs.storage]` e definido em `iroha_config` como `SorafsStorage` e e
  carregado o arquivo de configuração do nodo.
- `iroha_core` e `iroha_torii` passam a configuração de armazenamento para o construtor
  do gateway e o chunk store sem inicialização.
- Substituições de dev/test existentes (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), mas
  implantações de produção confiam no arquivo de configuração.

### Utilitários de CLI

Enquanto a superfície HTTP do Torii ainda está sendo ligada, o crate
`sorafs_node` envia uma nível CLI para que os operadores possam automatizar exercícios de
ingestão/exportação contra o backend persistente. [crates/sorafs_node/src/bin/sorafs-node.rs:1]

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` espera um manifesto `.to` codificado em Norito mais os bytes de carga útil
  correspondente. Ele reconstruiu o plano de pedaço a partir do perfil de
  chunking do manifest, impoe paridade de digest, persiste arquivos de chunk e
  opcionalmente emite um blob JSON `chunk_fetch_specs` para que o conjunto de ferramentas downstream
  valide o layout.
- `export` aceita um ID de manifesto e gravação de manifesto/carga útil armazenado em
  disco (com plano JSON opcional) para que fixtures continuem reproduzíveis.

Ambos os comandos imprimem um resumo Norito JSON em stdout, facilitando o uso em
roteiros. A CLI e coberta por um teste de integração para garantir que se manifeste e
payloads fazem roundtrip corretamente antes das APIs Torii entrarem. [crates/sorafs_node/tests/cli.rs:1]> Paridade HTTP
>
> O gateway Torii agora expoe helpers read-only suportados pelo mesmo
> `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` - retorna o manifesto
> Norito armazenado (base64) junto com resumo/metadados. [crates/iroha_torii/src/sorafs/api.rs:1207]
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` - retorna o plano de pedaço
> determinístico JSON (`chunk_fetch_specs`) para ferramentas downstream. [crates/iroha_torii/src/sorafs/api.rs:1259]
>
> Esses endpoints refletem a saida do CLI para que pipelines possam trocar
> scripts locais por probes HTTP sem alterar analisadores. [crates/iroha_torii/src/sorafs/api.rs:1207] [crates/iroha_torii/src/sorafs/api.rs:1259]

### Ciclo de vida do nodo

1. **Inicialização**:
   - Se o armazenamento estiver habilitado o nodo inicializa o chunk store com o
     diretorio e capacidade configurada. Isso inclui verificar ou criar uma base
     de dados de manifesto PoR e fazer replay de manifestos pinados para aquecer
     caches.
   - Registrador de rotas do gateway SoraFS (endpoints Norito JSON POST/GET para pin,
     buscar, amostragem PoR, telemetria).
   - Iniciar o trabalhador de amostragem PoR e o monitor de cotas.
2. **Descoberta/Anúncios**:
   - Gerar documentos `ProviderAdvertV1` usando capacidade/saúde atual, atualizar
     com a chave aprovada pelo conselho e publicar via discovery. Use uma lista
3. **Fluxo de pino**:
   - O gateway recebe um manifesto contratado (incluindo plano de chunk, raiz PoR,
     assinaturas do conselho). Validar uma lista de aliases (`sorafs.sf1@1.0.0`
     requerido) e garante que o plano de chunk corresponda aos metadados do manifesto.
   - Verifique cotas. Se capacidade/limites de pin excederem, responda com erro
     de política (Norito estruturado).
   - Stream de dados de chunk para o `ChunkStore`, verificando resumos durante
     uma ingestão. Atualiza árvores PoR e armazena metadados do manifesto no registro.
4. **Fluxo de busca**:
   - Servir requisições de gama de pedaços a partir da discoteca. O agendador impoe
     `max_parallel_fetches` e retorna `429` quando saturado.
   - Emite telemetria estruturada (Norito JSON) com latência, bytes servidos e
     contadores de erro para monitoramento downstream.
5. **Amostragem PoR**:
   - O trabalhador seleciona manifestos proporcionalmente ao peso (por exemplo, bytes
     armazenados) e roda amostragem determinística usando a árvore PoR do chunk store.
   - Persistir resultados para auditorias de governança e incluir resumos em anúncios
     de provedor / endpoints de telemetria.
6. **Evicção/aplicação de cotas**:
   - Quando a capacidade e atingida o nodo rejeita novos pinos por padrão. Opcionalmente,
     Os operadores podem configurar políticas de despejo (ex: TTL, LRU) quando o modelo
     de governança esteja definida; por ora o design assumir cotas estritas e
     operações de desbloqueio iniciadas pelo operador.

### Declaração de capacidade e integração de agendamento- Torii agora repassa atualizações de `CapacityDeclarationRecord` de `/v1/sorafs/capacity/declare`
  para o `CapacityManager` embutido, de modo que cada nodo constrói uma visão em memória
  de suas alocações comprometidas de chunker e lane. O manager expoe snapshots somente leitura
  para telemetria (`GET /v1/sorafs/capacity/state`) e impoe reservas por perfil ou pista
  antes de novos pedidos serem aceitos. [crates/sorafs_node/src/capacity.rs:1] [crates/sorafs_node/src/lib.rs:60]
- O endpoint `/v1/sorafs/capacity/schedule` aceita cargas úteis `ReplicationOrderV1`
  emitidos pela governança. Quando a ordem mira o provedor local o manager verifica
  agendamento duplicado, valida capacidade de chunker/lane, reserva o slice e retorna
  um `ReplicationPlan` descrevendo a capacidade restante para que ferramentas de
  orquestracao pode seguir com a ingestão. Ordens para outros provedores são
  reconhecidos com resposta `ignored` para facilitar fluxos de trabalho multi-operador. [crates/iroha_torii/src/routing.rs:4845]
- Hooks de conclusão (por exemplo, disparados após ingestão bem sucedida) chamam
  `POST /v1/sorafs/capacity/complete` para liberar reservas via
  `CapacityManager::complete_order`. A resposta inclui um instantâneo
  `ReplicationRelease` (totais restantes, resíduos de chunker/lane) para que ferramental
  de orquestracao possa enfileirar a proxima ordem sem polling. Trabalho futuro
  vai conectar isso ao pipeline do chunk store assim que a lógica de ingestão chegar.
  [crates/iroha_torii/src/routing.rs:4885] [crates/sorafs_node/src/capacity.rs:90]
- O `TelemetryAccumulator` embutido pode ser modificado via
  `NodeHandle::update_telemetry`, permitindo que trabalhadores de registro em segundo plano
  amostras de PoR/uptime e eventualmente derivar payloads canônicos
  `CapacityTelemetryV1` sem tocar nos internos do agendador. [crates/sorafs_node/src/lib.rs:142] [crates/sorafs_node/src/telemetry.rs:1]

### Integrações e trabalho futuro

- **Governanca**: extensão `sorafs_pin_registry_tracker.md` com telemetria de
  armazenamento (taxa de sucesso PoR, utilização de disco). Políticas de admissão podem
  exigência de capacidade mínima ou taxa mínima de sucesso PoR antes de aceitar anúncios.
- **SDKs de cliente**: exporta uma nova configuração de armazenamento (limites de disco, alias)
  para que ferramentas de gerenciamento possam inicializar nós programaticamente.
- **Telemetria**: integrar com a pilha de métricas existente (Prometheus /
  OpenTelemetry) para que métricas de armazenamento apareçam em dashboards de observabilidade.
- **Segurança**: rodar o módulo de armazenamento em um pool dedicado de tarefas async com
  contrapressão e considere sandboxing de leituras de chunk via io_uring ou pools
  tokio limitados para evitar que clientes maliciosos esgotem recursos.

Este design mantém o módulo de armazenamento opcional e determinístico enquanto fornece
os botões necessários para que os operadores participem da camada de disponibilidade de dados
SoraFS. Implemente mudanças necessárias em `iroha_config`, `iroha_core`, `iroha_torii`
e no gateway Norito, além do ferramental do anúncio do provedor.