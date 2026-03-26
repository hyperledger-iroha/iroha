---
lang: pt
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Modelo de dados Iroha v2 – Aprofundamento

Este documento explica as estruturas, identificadores, características e protocolos que formam o modelo de dados Iroha v2, conforme implementado na caixa `iroha_data_model` e usado em todo o espaço de trabalho. O objetivo é ser uma referência precisa que você possa revisar e propor atualizações.

## Escopo e Fundamentos

- Objetivo: Fornecer tipos canônicos para objetos de domínio (domínios, contas, ativos, NFTs, funções, permissões, pares), instruções de mudança de estado (ISI), consultas, gatilhos, transações, blocos e parâmetros.
- Serialização: todos os tipos públicos derivam codecs Norito (`norito::codec::{Encode, Decode}`) e esquema (`iroha_schema::IntoSchema`). JSON é usado seletivamente (por exemplo, para cargas úteis HTTP e `Json`) atrás de sinalizadores de recursos.
- Observação IVM: Certas validações de tempo de desserialização são desabilitadas ao direcionar a máquina virtual Iroha (IVM), pois o host executa a validação antes de invocar contratos (consulte a documentação da caixa em `src/lib.rs`).
- Portas FFI: alguns tipos são anotados condicionalmente para FFI via `iroha_ffi` atrás de `ffi_export`/`ffi_import` para evitar sobrecarga quando o FFI não é necessário.

## Principais características e ajudantes- `Identifiable`: As entidades possuem um `Id` e `fn id(&self) -> &Self::Id` estáveis. Deve ser derivado com `IdEqOrdHash` para facilidade de mapa/definição.
- `Registrable`/`Registered`: muitas entidades (por exemplo, `Domain`, `AssetDefinition`, `Role`) usam um padrão de construtor. `Registered` vincula o tipo de tempo de execução a um tipo de construtor leve (`With`) adequado para transações de registro.
- `HasMetadata`: Acesso unificado a um mapa chave/valor `Metadata`.
- `IntoKeyValue`: Auxiliar de divisão de armazenamento para armazenar `Key` (ID) e `Value` (dados) separadamente para reduzir a duplicação.
- `Owned<T>`/`Ref<'world, K, V>`: Wrappers leves usados ​​em storages e filtros de consulta para evitar cópias desnecessárias.

## Nomes e identificadores- `Name`: Identificador textual válido. Não permite espaços em branco e caracteres reservados `@`, `#`, `$` (usados ​​em IDs compostos). Construtível via `FromStr` com validação. Os nomes são normalizados para Unicode NFC na análise (grafias canonicamente equivalentes são tratadas como idênticas e armazenadas compostas). O nome especial `genesis` é reservado (verificado sem distinção entre maiúsculas e minúsculas).
- `IdBox`: um envelope do tipo soma para qualquer ID compatível (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, `PeerId`, `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Útil para fluxos genéricos e codificação Norito como um único tipo.
- `ChainId`: Identificador de cadeia opaca utilizado para proteção de replay em transações.Formas de string de IDs (ida e volta com `Display`/`FromStr`):
- `DomainId`: `name` (por exemplo, `wonderland`).
- `AccountId`: identificador canônico de conta sem domínio codificado via `AccountAddress` apenas como i105. As entradas do analisador devem ser canônicas i105; sufixos de domínio (`@domain`), literais canônicos i105, literais de alias, entrada de analisador hexadecimal canônico, cargas úteis `norito:` herdadas e formulários de analisador de conta `uaid:`/`opaque:` são rejeitados.
- `AssetDefinitionId`: `unprefixed Base58 address with versioning and checksum` canônico (bytes UUID-v4).
- `AssetId`: literal codificado canônico `<canonical-base58-asset-definition-id>` (formulários textuais legados não são suportados na primeira versão).
- `NftId`: `nft$domain` (por exemplo, `rose$garden`).
- `PeerId`: `public_key` (a igualdade entre pares é por chave pública).

## Entidades

### Domínio
- `DomainId { name: Name }` – nome exclusivo.
-`Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- Construtor: `NewDomain` com `with_logo`, `with_metadata`, então `Registrable::build(authority)` define `owned_by`.### Conta
- `AccountId` é a identidade canônica da conta sem domínio codificada pelo controlador e codificada como i105 canônico.
- `ScopedAccountId { account: AccountId, domain: DomainId }` transporta contexto de domínio explícito apenas quando uma visão com escopo definido é necessária.
- `Account { id, metadata, label?, uaid? }` — `label` é um alias estável opcional usado por registros de rechave, `uaid` carrega o [ID de conta universal] opcional em todo o Nexus (./universal_accounts_guide.md).
- Construtor: `NewAccount` via `Account::new(id)`; o registro requer um domínio `ScopedAccountId` explícito e não infere nenhum dos padrões.

### Definições de ativos e ativos
- `AssetDefinitionId { aid_bytes: [u8; 16] }` exposto textualmente como `unprefixed Base58 address`.
-`AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`; alias selectors resolve against latest committed block time and stop resolving after grace, while direct reads may still show `expired_pending_cleanup` until sweep.
  - `name` é um texto de exibição voltado para humanos e não deve conter `#`/`@`.
  - `alias` é opcional e deve ser um dos seguintes:
    -`<name>#<domain>.<dataspace>`
    -`<name>#<dataspace>`
    com o segmento esquerdo correspondendo exatamente a `AssetDefinition.name`.
  -`Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Construtores: `AssetDefinition::new(id, spec)` ou conveniência `numeric(id)`; `name` é necessário e deve ser definido via `.with_name(...)`.
-`AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Numeric }` com `AssetEntry`/`AssetValue` de fácil armazenamento.
- `AssetBalanceScope`: `Global` para saldos irrestritos e `Dataspace(DataSpaceId)` para saldos restritos a espaço de dados.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` exposto para APIs de resumo.### NFTs
-`NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (o conteúdo são metadados de chave/valor arbitrários).
- Construtor: `NewNft` via `Nft::new(id, content)`.

### Funções e permissões
-`RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` com construtor `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }` – o `name` e o esquema de carga útil devem estar alinhados com o `ExecutorDataModel` ativo (veja abaixo).

### Pares
-`PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` e forma de string `public_key@address` analisável.

### Primitivas criptográficas (recurso `sm`)
- `Sm2PublicKey` e `Sm2Signature`: pontos compatíveis com SEC1 e assinaturas `r∥s` de largura fixa para SM2. Os construtores validam a associação da curva e os IDs distintivos; A codificação Norito reflete a representação canônica usada por `iroha_crypto`.
- `Sm3Hash`: novo tipo `[u8; 32]` que representa o resumo GM/T 0004, usado em manifestos, telemetria e respostas de syscall.
- `Sm4Key`: wrapper de chave simétrica de 128 bits compartilhado entre syscalls de host e acessórios de modelo de dados.
Esses tipos ficam ao lado das primitivas Ed25519/BLS/ML-DSA existentes e se tornam parte do esquema público quando o espaço de trabalho é construído com `--features sm`.### Gatilhos e eventos
-`TriggerId { name: Name }` e `Trigger { id, action: action::Action }`.
-`action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` ou `Exactly(u32)`; utilitários de pedido e esgotamento incluídos.
  - Segurança: `TriggerCompleted` não pode ser usado como filtro de ação (validado durante a (des)serialização).
- `EventBox`: tipo de soma para eventos de pipeline, lote de pipeline, dados, tempo, gatilho de execução e gatilho concluído; `EventFilterBox` reflete isso para assinaturas e filtros de gatilho.

## Parâmetros e configuração

- Famílias de parâmetros do sistema (todos `Default`ed, carregam getters e convertem em enums individuais):
-`SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  -`BlockParameters { max_transactions: NonZeroU64 }`.
  -`TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  -`SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` agrupa todas as famílias e um `custom: BTreeMap<CustomParameterId, CustomParameter>`.
- Enums de parâmetro único: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` para atualizações e iterações semelhantes a diferenças.
- Parâmetros personalizados: definidos pelo executor, transportados como `Json`, identificados por `CustomParameterId` (um `Name`).

## ISI (Iroha Instruções Especiais)- Característica principal: `Instruction` com `dyn_encode`, `as_any` e um identificador estável por tipo `id()` (o padrão é o nome do tipo concreto). Todas as instruções são `Send + Sync + 'static`.
- `InstructionBox`: wrapper `Box<dyn Instruction>` de propriedade com clone/eq/ord implementado via tipo ID + bytes codificados.
- As famílias de instruções integradas são organizadas em:
  - `mint_burn`, `transfer`, `register` e um pacote de ajudantes `transparent`.
  - Digite enums para metafluxos: `InstructionType`, somas em caixas como `SetKeyValueBox` (domínio/conta/asset_def/nft/trigger).
- Erros: modelo de erro rico sob `isi::error` (erros de tipo de avaliação, erros de localização, capacidade de cunhagem, matemática, parâmetros inválidos, repetição, invariantes).
- Registro de instruções: a macro `instruction_registry!{ ... }` cria um registro de decodificação de tempo de execução codificado por nome de tipo. Usado pelo clone `InstructionBox` e pelo serde Norito para obter (des)serialização dinâmica. Se nenhum registro tiver sido definido explicitamente por meio de `set_instruction_registry(...)`, um registro padrão integrado com todo o ISI principal será instalado preguiçosamente no primeiro uso para manter os binários robustos.

## Transações- `Executable`: `Instructions(ConstVec<InstructionBox>)` ou `Ivm(IvmBytecode)`. `IvmBytecode` serializa como base64 (novo tipo transparente sobre `Vec<u8>`).
- `TransactionBuilder`: constrói uma carga útil de transação com `chain`, `authority`, `creation_time_ms`, `time_to_live_ms` opcional e `nonce`, `metadata` e um `Executable`.
  - Ajudantes: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_creation_time`, `sign`.
- `SignedTransaction` (versionado com `iroha_version`): carrega `TransactionSignature` e carga útil; fornece hashing e verificação de assinatura.
- Pontos de entrada e resultados:
  -`TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` com auxiliares de hash.
  - `ExecutionStep(ConstVec<InstructionBox>)`: um único lote ordenado de instruções em uma transação.

## Blocos- `SignedBlock` (versionado) encapsula:
  - `signatures: BTreeSet<BlockSignature>` (de validadores),
  -`payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (estado de execução secundário) contendo `time_triggers`, árvores Merkle de entrada/resultado, `transaction_results` e `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- Utilitários: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `add_signature`, `replace_signatures`.
- Raízes Merkle: os pontos de entrada e resultados da transação são confirmados por meio de árvores Merkle; resultado A raiz Merkle é colocada no cabeçalho do bloco.
- As provas de inclusão de bloco (`BlockProofs`) expõem as provas Merkle de entrada/resultado e o mapa `fastpq_transcripts` para que os provadores fora da cadeia possam buscar os deltas de transferência associados a um hash de transação.
- Mensagens `ExecWitness` (transmitidas via Torii e apoiadas em fofocas de consenso) agora incluem `fastpq_transcripts` e `fastpq_batches: Vec<FastpqTransitionBatch>` pronto para prova com `public_inputs` incorporado (dsid, slot,roots, perm_root, tx_set_hash), portanto, provadores externos podem ingerir linhas canônicas do FASTPQ sem recodificar as transcrições.

## Consultas- Dois sabores:
  - Singular: implementar `SingularQuery<Output>` (por exemplo, `FindParameters`, `FindExecutorDataModel`).
  - Iterável: implemente `Query<Item>` (por exemplo, `FindAccounts`, `FindAssets`, `FindDomains`, etc.).
- Formulários apagados:
  - `QueryBox<T>` é um `Query<Item = T>` in a box e apagado com serde Norito apoiado por um registro global.
  - `QueryWithFilter<T> { query, predicate, selector }` emparelha uma consulta com um predicado/seletor DSL; converte em uma consulta iterável apagada via `From`.
- Registro e codecs:
  - `query_registry!{ ... }` cria um registro global mapeando tipos de consulta concretos para construtores por nome de tipo para decodificação dinâmica.
  -`QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` e `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` é um tipo de soma sobre vetores homogêneos (por exemplo, `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), além de auxiliares de tupla e extensão para paginação eficiente.
- DSL: implementado em `query::dsl` com características de projeção (`HasProjection<PredicateMarker>`/`SelectorMarker`) para predicados e seletores verificados em tempo de compilação. Um recurso `fast_dsl` expõe uma variante mais leve, se necessário.

## Executor e extensibilidade- `Executor { bytecode: IvmBytecode }`: o pacote de código executado pelo validador.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` declara o domínio definido pelo executor:
  - Parâmetros de configuração personalizados,
  - Identificadores de instruções personalizados,
  - Identificadores de token de permissão,
  - Um esquema JSON que descreve tipos personalizados para ferramentas do cliente.
- Existem amostras de personalização em `data_model/samples/executor_custom_data_model` demonstrando:
  - Token de permissão personalizado via derivação `iroha_executor_data_model::permission::Permission`,
  - Parâmetro personalizado definido como um tipo conversível em `CustomParameter`,
  - Instruções personalizadas serializadas em `CustomInstruction` para execução.

### CustomInstruction (ISI definido pelo executor)- Tipo: `isi::CustomInstruction { payload: Json }` com identificação de fio estável `"iroha.custom"`.
- Finalidade: envelope para instruções específicas do executor em redes privadas/consorciadas ou para prototipagem, sem bifurcar o modelo de dados público.
- Comportamento padrão do executor: o executor integrado em `iroha_core` não executa `CustomInstruction` e entrará em pânico se for encontrado. Um executor personalizado deve fazer downcast de `InstructionBox` para `CustomInstruction` e interpretar deterministicamente a carga útil em todos os validadores.
- Norito: codifica/decodifica via `norito::codec::{Encode, Decode}` com esquema incluído; a carga útil `Json` é serializada de forma determinística. As viagens de ida e volta são estáveis ​​desde que o registro de instruções inclua `CustomInstruction` (faz parte do registro padrão).
- IVM: Kotodama compila para o bytecode IVM (`.to`) e é o caminho recomendado para a lógica do aplicativo. Use `CustomInstruction` apenas para extensões em nível de executor que ainda não podem ser expressas em Kotodama. Garanta o determinismo e binários de executores idênticos entre pares.
- Não para redes públicas: não use para cadeias públicas onde executores heterogêneos correm o risco de bifurcações de consenso. Prefira propor um novo upstream ISI integrado quando precisar de recursos de plataforma.

## Metadados- `Metadata(BTreeMap<Name, Json>)`: armazenamento de chave/valor anexado a múltiplas entidades (`Domain`, `Account`, `AssetDefinition`, `Nft`, gatilhos e transações).
- API: `contains`, `iter`, `get`, `insert` e (com `transparent_api`) `remove`.

## Características e Determinismo

- Recursos controlam APIs opcionais (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, `http`, `fault_injection`).
- Determinismo: toda serialização usa codificação Norito para ser portátil em hardware. O bytecode IVM é um blob de bytes opaco; a execução não deve introduzir reduções não determinísticas. O host valida transações e fornece entradas para IVM de forma determinística.

### API transparente (`transparent_api`)- Objetivo: expõe acesso completo e mutável às estruturas/enums `#[model]` para componentes internos, como Torii, executores e testes de integração. Sem ele, esses itens são intencionalmente opacos, de modo que os SDKs externos veem apenas construtores seguros e cargas codificadas.
- Mecânica: a macro `iroha_data_model_derive::model` reescreve cada campo público com `#[cfg(feature = "transparent_api")] pub` e mantém uma cópia privada para a construção padrão. A ativação do recurso inverte esses cfgs, portanto, a desestruturação de `Account`, `Domain`, `Asset`, etc., torna-se legal fora de seus módulos de definição.
- Detecção de superfície: a caixa exporta uma constante `TRANSPARENT_API: bool` (gerada em `transparent_api.rs` ou `non_transparent_api.rs`). O código downstream pode verificar esse sinalizador e ramificar quando precisar recorrer a auxiliares opacos.
- Habilitando: adicione `features = ["transparent_api"]` à dependência em `Cargo.toml`. As caixas do espaço de trabalho que precisam da projeção JSON (por exemplo, `iroha_torii`) encaminham o sinalizador automaticamente, mas os consumidores terceiros devem mantê-lo desativado, a menos que controlem a implantação e aceitem a superfície mais ampla da API.

## Exemplos rápidos

Crie um domínio e uma conta, defina um ativo e crie uma transação com instruções:

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(kp.public_key().clone());
let new_account = Account::new(account_id.to_account_id(domain_id.clone()))
    .with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_name("USD Coin".to_owned())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

Consulte contas e ativos com o DSL:

```rust
use iroha_data_model::prelude::*;

let predicate = query::dsl::CompoundPredicate::build(|p| {
    p.equals("metadata.tier", 1_u32)
        .exists("metadata.display_name")
});
let selector = query::dsl::SelectorTuple::default();
let q: QueryBox<QueryOutputBatchBox> =
    QueryWithFilter::new(
        Box::new(query::account::FindAccounts),
        predicate,
        selector,
    ).into();
// Encode and send via Torii; decode on server using the query registry
```

Use o bytecode do contrato inteligente IVM:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

asset-definition id / referência rápida de alias (CLI + Torii):

```bash
# Register an asset definition with canonical Base58 id + explicit name + alias
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#ubl.sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components (no manual norito hex copy/paste)
iroha ledger asset mint \
  --definition-alias pkr#ubl.sbp \
  --account sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB \
  --quantity 500

# Resolve alias to canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl.sbp"}'
```Nota de migração:
- IDs de definição de ativos `name#domain` antigos não são aceitos na v1.
- Os IDs de ativos para criação/gravação/transferência permanecem canônicos `<canonical-base58-asset-definition-id>`; construí-los com:
  -`iroha tools encode asset-id --definition <base58-asset-definition-id> --account <i105>`
  - ou `--alias <name>#<domain>.<dataspace>` / `--alias <name>#<dataspace>` + `--account`.

## Versionamento

- `SignedTransaction`, `SignedBlock` e `SignedQuery` são estruturas canônicas codificadas em Norito. Cada um implementa `iroha_version::Version` para prefixar sua carga útil com a versão ABI atual (atualmente `1`) quando codificado via `EncodeVersioned`.

## Notas de revisão/atualizações potenciais

- Consultar DSL: considere documentar um subconjunto estável voltado para o usuário e exemplos de filtros/seletores comuns.
- Famílias de instruções: expanda os documentos públicos listando as variantes ISI integradas expostas por `mint_burn`, `register`, `transfer`.

---
Se alguma parte precisar de mais profundidade (por exemplo, catálogo ISI completo, lista completa de registros de consultas ou campos de cabeçalho de bloco), avise-me e estenderei essas seções de acordo.