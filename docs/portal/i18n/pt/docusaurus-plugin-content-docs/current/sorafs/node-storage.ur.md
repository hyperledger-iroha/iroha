---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-storage.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: armazenamento de nó
título: Projeto de armazenamento de nó SoraFS
sidebar_label: design de armazenamento de nó
descrição: Host de dados SoraFS کرنے والے Nós Torii کے لیے arquitetura de armazenamento, cotas e ganchos de ciclo de vida۔
---

:::nota مستند ماخذ
:::

## Projeto de armazenamento de nó SoraFS (rascunho)

یہ نوٹ واضح کرتا ہے کہ Nó Iroha (Torii) کس طرح SoraFS camada de disponibilidade de dados میں opt-in کر سکتا ہے اور disco local کا ایک حصہ pedaços ذخیرہ/سرو کرنے کے لیے مختص کر سکتا ہے۔ یہ `sorafs_node_client_protocol.md` especificação de descoberta e dispositivo SF-1b کام کی تکمیل کرتا ہے, arquitetura do lado do armazenamento, controles de recursos, encanamento de configuração بیان کرتا ہے nó ou caminhos de código de gateway میں شامل ہونا ضروری ہے۔ عملی آپریشنل brocas
[Node Operations Runbook](./node-operations) میں موجود ہیں۔

### Metas

- کسی بھی validador یا processo auxiliar Iroha کو disco sobressalente بطور SoraFS exposição do provedor کرنے دینا بغیر core ledger ذمہ داریاں متاثر کیے۔
- módulo de armazenamento کو determinístico اور Norito رکھنا: manifestos, planos de blocos, raízes de prova de recuperação (PoR), اور anúncios de provedores ہی fonte de verdade ہیں۔
- cotas definidas pelo operador impõem کرنا تاکہ nó بہت زیادہ solicitações pin/fetch قبول کر کے اپنے recursos ختم نہ کر لے۔
- saúde/telemetria (amostragem PoR, latência de busca de pedaços, pressão de disco) کو governança اور clientes تک واپس پہنچانا۔

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

Módulos principais:

- **Gateway**: endpoints Norito HTTP expõem propostas de pin, solicitações de busca de blocos, amostragem PoR e telemetria. Cargas úteis Norito validam کرتا ہے اور solicitações کو chunk store میں marechal کرتا ہے۔ O daemon que você usa é o Torii Reutilização de pilha HTTP.
- **Registro de Pins**: estado do pin manifesto جو `iroha_data_model::sorafs` اور `iroha_core` میں track ہوتی ہے۔ Aceitação de manifesto ہونے پر registro manifest digest, chunk plan digest, raiz PoR e capacidade do provedor sinaliza registro کرتا ہے۔
- **Armazenamento de blocos**: implementação `ChunkStore` apoiada em disco e ingestão de manifestos assinados کرتی ہے، `ChunkProfile::DEFAULT` سے planos de blocos se materializam کرتی ہے, اور pedaços کو layout determinístico میں persistem کرتی ہے۔ ہر impressão digital de conteúdo de pedaço اور metadados PoR کے ساتھ associar ہوتا ہے تاکہ amostragem بغیر پورا فائل دوبارہ پڑھنے کے revalidar ہو سکے۔
- **Cota/Agendador**: limites configurados pelo operador (máximo de bytes de disco, máximo de pinos pendentes, máximo de buscas paralelas, TTL de bloco) impor کرتا ہے اور IO کو coordenar کرتا ہے تاکہ nó کے deveres de razão morrerem de fome نہ ہوں۔ Provas PoR do agendador e solicitações de amostragem کو CPU limitada کے ساتھ servir بھی کرتا ہے۔

### Configuração

`iroha_config` میں نیا seção شامل کریں:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # optional human friendly tag
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```- `enabled`: alternância de participação۔ false ہو تو pontos de extremidade de armazenamento de gateway پر 503 دیتا ہے اور descoberta de nó میں anunciar نہیں کرتا۔
- `data_dir`: dados de pedaços, árvores PoR e busca de telemetria کے لیے diretório raiz۔ Padrão `<iroha.data_dir>/sorafs` ہے۔
- `max_capacity_bytes`: dados de pedaços fixados کے لیے limite rígido۔ limite de tarefas em segundo plano پر پہنچنے پر نئے pinos rejeitados کرتی ہے۔
- `max_parallel_fetches`: agendador کا limite de simultaneidade جو largura de banda/IO de disco کو carga de trabalho do validador کے ساتھ equilíbrio کرتا ہے۔
- `max_pins`: pinos manifestos کی máximo تعداد جو nó قبول کرتا ہے اس سے پہلے despejo/contrapressão aplicada ہو۔
- `por_sample_interval_secs`: trabalhos automáticos de amostragem PoR کی cadência۔ ہر trabalho `N` deixa amostra کرتا ہے (por manifesto configurável) اور eventos de telemetria emitem کرتا ہے۔ Chave de metadados de governança `profile.sample_multiplier` (inteiro `1-4`) سے `N` کو escala deterministicamente کر سکتی ہے۔ Valor ایک número único/string یا substituições por perfil والا objeto ہو سکتا ہے، مثلاً `{"default":2,"sorafs.sf2@1.0.0":3}`۔
- `adverts`: estrutura e gerador de anúncio do provedor Campos `ProviderAdvertV1` (ponteiro de aposta, dicas de QoS, tópicos) بھرنے کے لیے استعمال کرتا ہے۔ اگر omitir ہو تو padrões de registro de governança de nó استعمال کرتا ہے۔

Encanamento de configuração:

- `[sorafs.storage]` `iroha_config` میں `SorafsStorage` کے طور پر definir ہے اور arquivo de configuração do nó سے carregar ہوتا ہے۔
- `iroha_core` e `iroha_torii` configuração de armazenamento کو inicialização پر gateway builder اور chunk store میں thread کرتے ہیں۔
- Substituições de ambiente de desenvolvimento/teste موجود ہیں (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), مگر implantações de produção e arquivo de configuração پر confiar کرنا چاہیے۔

### Utilitários CLI

جب تک Torii کی HTTP superfície wire ہو رہی ہے, `sorafs_node` crate ایک thin CLI فراہم کرتا ہے تاکہ operadores persistente backend کے خلاف script de exercícios de ingestão/exportação کر سکیں۔【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` Norito manifesto codificado `.to` اور bytes de carga útil correspondentes esperados کرتا ہے۔ یہ manifesto کے perfil de chunking سے reconstrução do plano de chunk کرتا ہے، digerir paridade impor کرتا ہے, arquivos de chunk persistem کرتا ہے، اور opcionalmente `chunk_fetch_specs` JSON blob emit کرتا ہے تاکہ downstream verificação de integridade do layout das ferramentas
- `export` ID do manifesto قبول کرتا ہے اور manifesto/carga útil armazenada کو disco پر لکھتا ہے (plano opcional JSON کے ساتھ) تاکہ ambientes de fixtures میں reproduzíveis رہیں۔

Os comandos stdout são Norito Resumo JSON Teste de integração CLI سے coberto ہے تاکہ manifestos/cargas úteis ida e volta درست رہیں جب تک APIs Torii نہ آئیں۔【crates/sorafs_node/tests/cli.rs:1】> Paridade HTTP
>
> Gateway Torii اب ajudantes somente leitura expõem کرتا ہے جو اسی `NodeHandle` پر baseado em ہیں:
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — resumo/metadados do manifesto Norito armazenado (base64) کے ساتھ واپس کرتا ہے۔【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — plano de bloco determinístico JSON (`chunk_fetch_specs`) ferramentas downstream
>
> یہ saída CLI de endpoints کو espelho کرتے ہیں تاکہ pipelines scripts locais سے sondas HTTP پر بغیر analisador بدلے منتقل ہو سکیں۔【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Ciclo de vida do nó

1. **Inicialização**:
   - armazenamento habilitado ہو تو diretório configurado no nó اور capacidade کے ساتھ inicialização de armazenamento de blocos کرتا ہے۔ اس میں Banco de dados de manifesto PoR verificar/criar کرنا اور repetição de manifestos fixados کر کے caches quentes کرنا شامل ہے۔
   - Registro de rotas de gateway SoraFS کریں (terminais Norito JSON POST/GET para pin, fetch, amostra PoR, telemetria)۔
   - Trabalhador de amostragem PoR e monitor de cota spawn کریں۔
2. **Descoberta/Anúncios**:
3. **Fixar fluxo de trabalho**:
   - Manifesto assinado pelo gateway وصول کرتا ہے (plano de bloco, raiz PoR, assinaturas do conselho شامل)۔ lista de alias validar کریں (`sorafs.sf1@1.0.0` obrigatório) اور plano de bloco کو metadados de manifesto سے correspondência کریں۔
   - Verificação de cotas کریں۔ Os limites de capacidade/pin excedem ہوں تو erro de política (Norito estruturado) دیں۔
   - Dados de pedaços کو `ChunkStore` میں stream کریں اور ingest کے دوران digests verify کریں۔ Atualização de árvores PoR کریں e registro de metadados de manifesto میں armazenamento کریں۔
4. **Fluxo de trabalho de busca**:
   - As solicitações de intervalo de blocos do disco servem کریں۔ Agendador `max_parallel_fetches` impor کرتا ہے اور saturado ہونے پر `429` واپس کرتا ہے۔
   - Telemetria estruturada (Norito JSON) emite کریں جس میں latência, bytes servidos e contagens de erros ہوں تاکہ monitoramento downstream ہو۔
5. **Amostragem PoR**:
   - Manifestos de trabalho کو peso (مثلاً bytes armazenados) کے تناسب سے select کرتا ہے اور armazenamento de pedaços کے Árvore PoR سے amostragem determinística کرتا ہے۔
   - Resultados کو auditorias de governança کے لیے persistir کریں اور anúncios do provedor/endpoints de telemetria میں resumos شامل کریں۔
6. **Despejo/aplicação de cotas**:
   - Capacidade پہنچنے پر node default طور پر نئے pinos rejeitados کرتا ہے۔ Políticas opcionais de despejo de operadores (مثلاً TTL, LRU) configure کر سکتے ہیں جب modelo de governança طے ہو جائے؛ فی الحال projetar cotas rigorosas اور operações de liberação iniciadas pelo operador فرض کرتا ہے۔

### Declaração de capacidade e integração de agendamento- Torii اب `/v2/sorafs/capacity/declare` سے `CapacityDeclarationRecord` atualiza `CapacityManager` incorporado تک relé کرتا ہے, تاکہ ہر nó اپنی chunker comprometido/alocações de pista کا visualização na memória Telemetria do gerente کے لیے instantâneos somente leitura (`GET /v2/sorafs/capacity/state`) expor کرتا ہے اور نئے ordens قبول کرنے سے پہلے por perfil یا reservas por pista impor کرتا ہے۔【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- Cargas úteis `ReplicationOrderV1` emitidas por governança de endpoint `/v2/sorafs/capacity/schedule` قبول کرتا ہے۔ جب solicitar provedor local کو alvo کرے تو agendamento duplicado do gerente چیک کرتا ہے, verificação de capacidade do chunker/lane کرتا ہے, reserva de fatia کرتا ہے, اور `ReplicationPlan` واپس کرتا ہے جو capacidade restante بیان کرے تاکہ ingestão de ferramentas de orquestração جاری رکھ سکے۔ دوسرے provedores کے pedidos کو resposta `ignored` سے reconhecimento کیا جاتا ہے تاکہ fluxos de trabalho multi-operador آسان ہوں۔【crates/iroha_torii/src/routing.rs:4845】
- Ganchos de conclusão (مثلاً ingestão کامیاب ہونے کے بعد) `POST /v2/sorafs/capacity/complete` کو hit کرتے ہیں تاکہ `CapacityManager::complete_order` کے ذریعے reservas liberar Resposta میں Instantâneo `ReplicationRelease` (totais restantes, resíduos de chunker/pista) یہ بعد میں pipeline de armazenamento de pedaços کے ساتھ fio ہوگا جب terreno lógico de ingestão ہو جائے۔【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- `TelemetryAccumulator` incorporado کو `NodeHandle::update_telemetry` کے ذریعے mutate کیا جا سکتا ہے، جس سے trabalhadores em segundo plano registro de amostras PoR/uptime کرتے ہیں اور مستقبل میں cargas úteis canônicas `CapacityTelemetryV1` derivam کر سکتے ہیں بغیر internos do agendador چھیڑے۔【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integrações e trabalhos futuros

- **Governança**: `sorafs_pin_registry_tracker.md` کو telemetria de armazenamento (taxa de sucesso de PoR, utilização de disco) کے ساتھ estender کریں۔ Os anúncios de políticas de admissão aceitam کرنے سے پہلے capacidade mínima یا taxa mínima de sucesso de PoR exigida کر سکتی ہیں۔
- **SDKs do cliente**: configuração de armazenamento (limites de disco, alias) expõe ferramentas de gerenciamento de forma programática, inicialização de nós کر سکے۔
- **Telemetria**: pilha de métricas de armazenamento (Prometheus / OpenTelemetry) کے ساتھ integrar کریں تاکہ painéis de observabilidade de métricas de armazenamento
- **Segurança**: módulo de armazenamento کو pool de tarefas assíncronas dedicado میں contrapressão کے ساتھ چلائیں اور leituras de pedaços کو io_uring یا pools de tokio limitados کے ذریعے sandbox کرنے پر غور کریں تاکہ esgotamento de recursos de clientes maliciosos نہ کر سکیں۔

یہ módulo de armazenamento de design کو opcional اور determinístico رکھتا ہے جبکہ operadores کو SoraFS camada de disponibilidade de dados میں حصہ لینے کے لیے ضروری botões فراہم کرتا ہے۔ Para obter o gateway `iroha_config`, `iroha_core`, `iroha_torii` ou Norito, você pode usar o gateway ferramentas de anúncio de provedor