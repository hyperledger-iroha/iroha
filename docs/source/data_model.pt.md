<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8055b28096f5884d2636a19a98e92a74599802fa1bd3ff350dbb636d1300b1f8
source_last_modified: "2026-03-30T18:22:55.957443+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Modelo de dados Iroha v2 – Aprofundamento

Este documento explica as estruturas, identificadores, características e protocolos que formam o modelo de dados Iroha v2, conforme implementado na caixa `iroha_data_model` e usado em todo o espaço de trabalho. O objetivo é ser uma referência precisa que você possa revisar e propor atualizações.

## Escopo e Fundamentos

- Objetivo: Fornecer tipos canônicos para objetos de domínio (domínios, contas, ativos, NFTs, funções, permissões, pares), instruções de mudança de estado (ISI), consultas, gatilhos, transações, blocos e parâmetros.
- Serialização: todos os tipos públicos derivam codecs Norito (`norito::codec::{Encode, Decode}`) e esquema (`iroha_schema::IntoSchema`). JSON é usado seletivamente (por exemplo, para cargas úteis HTTP e `Json`) atrás de sinalizadores de recursos.
- Observação IVM: Certas validações de tempo de desserialização são desabilitadas ao direcionar a máquina virtual Iroha (IVM), pois o host executa a validação antes de invocar contratos (consulte a documentação da caixa em `src/lib.rs`).
- Portas FFI: alguns tipos são anotados condicionalmente para FFI via `iroha_ffi` atrás de `ffi_export`/`ffi_import` para evitar sobrecarga quando o FFI não é necessário.

## Principais características e ajudantes- `Identifiable`: As entidades possuem um `Id` e `fn id(&self) -> &Self::Id` estáveis. Deve ser derivado com `IdEqOrdHash` para facilitar o mapa/definir.
- `Registrable`/`Registered`: muitas entidades (por exemplo, `Domain`, `AssetDefinition`, `Role`) usam um padrão de construtor. `Registered` vincula o tipo de tempo de execução a um tipo de construtor leve (`With`) adequado para transações de registro.
- `HasMetadata`: Acesso unificado a um mapa chave/valor `Metadata`.
- `IntoKeyValue`: Auxiliar de divisão de armazenamento para armazenar `Key` (ID) e `Value` (dados) separadamente para reduzir a duplicação.
- `Owned<T>`/`Ref<'world, K, V>`: Wrappers leves usados ​​em storages e filtros de consulta para evitar cópias desnecessárias.

## Nomes e identificadores- `Name`: Identificador textual válido. Não permite espaços em branco e caracteres reservados `@`, `#`, `$` (usados ​​em IDs compostos). Construtível via `FromStr` com validação. Os nomes são normalizados para Unicode NFC na análise (grafias canonicamente equivalentes são tratadas como idênticas e armazenadas compostas). O nome especial `genesis` é reservado (verificado sem distinção entre maiúsculas e minúsculas).
- `IdBox`: um envelope do tipo soma para qualquer ID compatível (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, `PeerId`, `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Útil para fluxos genéricos e codificação Norito como um único tipo.
- `ChainId`: Identificador de cadeia opaca utilizado para proteção de replay em transações.Formas de string de IDs (ida e volta com `Display`/`FromStr`):
- `DomainId`: `name` (por exemplo, `wonderland`).
- `AccountId`: identificador canônico de conta sem domínio codificado via `AccountAddress` apenas como I105. As entradas do analisador estrito devem ser canônicas I105; sufixos de domínio (`@domain`), literais de alias de conta, entrada de analisador hexadecimal canônico, cargas úteis `norito:` legadas e formulários de analisador de conta `uaid:`/`opaque:` são rejeitados. Os aliases de contas na cadeia usam `name@domain.dataspace` ou `name@dataspace` e resolvem para valores canônicos `AccountId`.
- `AssetDefinitionId`: endereço Base58 canônico sem prefixo sobre os bytes canônicos de definição de ativo. Este é o ID do ativo público. Os aliases de ativos na cadeia usam `name#domain.dataspace` ou `name#dataspace` e resolvem apenas para este ID de ativo Base58 canônico.
- `AssetId`: identificador de ativo público em formato Base58 simples e canônico. Aliases de ativos como `name#dataspace` ou `name#domain.dataspace` são resolvidos como `AssetId`. Os acervos contábeis internos também podem expor campos `asset + account + optional dataspace` divididos quando necessário, mas esse formato composto não é o `AssetId` público.
- `NftId`: `nft$domain` (por exemplo, `rose$garden`).
- `PeerId`: `public_key` (a igualdade entre pares é por chave pública).

## Entidades### Domínio
- `DomainId { name: Name }` – nome exclusivo.
-`Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- Construtor: `NewDomain` com `with_logo`, `with_metadata`, então `Registrable::build(authority)` define `owned_by`.### Conta
- `AccountId` é a identidade canônica da conta sem domínio codificada pelo controlador e codificada como I105 canônico.
- `Account { id, metadata, label?, uaid?, opaque_ids[] }` - `label` é um `AccountAlias` primário opcional usado por registros de rechaveamento, `uaid` carrega as faixas opcionais Nexus [Universal Account ID] (./universal_accounts_guide.md) e `opaque_ids` identificadores ocultos vinculados a esse UAID. O estado da conta armazenada não contém mais nenhum campo de domínio vinculado.
- Construtores:
  - `NewAccount` via `Account::new(id)` registra o assunto canônico da conta sem domínio.
- Modelo alternativo:
  - A identidade da conta canônica nunca inclui um segmento de domínio ou espaço de dados.
  - Os valores `AccountAlias` são ligações SNS separadas em camadas sobre `AccountId`.
  - Aliases qualificados de domínio, como `merchant@banka.sbp`, carregam um domínio e um espaço de dados na ligação de alias.
  - Os aliases raiz do espaço de dados, como `merchant@sbp`, carregam apenas o espaço de dados e, portanto, emparelham naturalmente com `Account::new(...)`.
  - Os testes e acessórios devem propagar primeiro o `AccountId` universal e, em seguida, adicionar concessões de alias, permissões de alias e qualquer estado de propriedade do domínio separadamente, em vez de codificar suposições de domínio na própria identidade da conta.
  - A pesquisa de conta pública singular agora se concentra em aliases (`FindAliasesByAccountId`); a própria identidade da conta permanece sem domínio.### Definições de ativos e ativos
- `AssetDefinitionId { aid_bytes: [u8; 16] }` exposto textualmente como um endereço Base58 não prefixado com controle de versão e soma de verificação.
-`AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `name` é um texto de exibição voltado para humanos e não deve conter `#`/`@`.
  - `alias` é opcional e deve ser um dos seguintes:
    -`<name>#<domain>.<dataspace>`
    -`<name>#<dataspace>`
    com o segmento esquerdo correspondendo exatamente a `AssetDefinition.name`.
  - O estado de arrendamento do alias é armazenado com autoridade no registro de ligação de alias persistente; o campo `alias` embutido é derivado quando as definições são lidas por meio de APIs core/Torii.
  - As respostas de definição de ativo Torii podem incluir `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, onde `status` é um de `permanent`, `leased_active`, `leased_grace` ou `expired_pending_cleanup`.
  - A resolução do alias usa o carimbo de data/hora do bloco confirmado mais recente em vez do relógio de parede do nó. Após a passagem de `grace_until_ms`, os seletores de alias param de resolver imediatamente, mesmo que a limpeza de varredura ainda não tenha removido a ligação obsoleta; leituras de definição direta ainda podem relatar a ligação persistente como `expired_pending_cleanup`.
  -`Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Construtores: `AssetDefinition::new(id, spec)` ou conveniência `numeric(id)`; `name` é necessário e deve ser definido via `.with_name(...)`.
-`AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Numeric }` com `AssetEntry`/`AssetValue` de fácil armazenamento.- `AssetBalanceScope`: `Global` para saldos irrestritos e `Dataspace(DataSpaceId)` para saldos restritos a espaço de dados.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` exposto para APIs de resumo.

### NFTs
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
- `EventBox`: tipo de soma para eventos de pipeline, lote de pipeline, dados, tempo, gatilho de execução e gatilho concluído; `EventFilterBox` reflete isso para assinaturas e filtros de acionamento.

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
  - Parâmetro personalizado definido como tipo conversível em `CustomParameter`,
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
let new_account = Account::new(account_id.clone())
    .with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id = AssetDefinitionId::new(
    "wonderland".parse().unwrap(),
    "usd".parse().unwrap(),
);
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

Referência rápida de ID/alias de definição de ativo (CLI + Torii):

```bash
# Register an asset definition with a canonical Base58 id + explicit name + alias
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#bankb.sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components
iroha ledger asset mint \
  --definition-alias pkr#bankb.sbp \
  --account sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB \
  --quantity 500

# Resolve alias to the canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#bankb.sbp"}'
```Nota de migração:
- IDs de definição de ativos `name#domain` antigos não são aceitos na v1.
- Os seletores de ativos públicos usam apenas um formato de definição de ativos: ids canônicos Base58. Os aliases permanecem seletores opcionais, mas resolvem para o mesmo ID canônico.
- Pesquisas de ativos públicos abordam saldos próprios com `asset + account + optional scope`; Literais `AssetId` codificados brutos são uma representação interna e não fazem parte da superfície do seletor Torii/CLI.
- `POST /v1/assets/definitions/query` e `GET /v1/assets/definitions` aceitam filtros/classificações de definição de ativos em `alias_binding.status`, `alias_binding.lease_expiry_ms`, `alias_binding.grace_until_ms` e `alias_binding.bound_at_ms`, além de `id`, `name`, `alias` e `metadata.*`.

## Versionamento

- `SignedTransaction`, `SignedBlock` e `SignedQuery` são estruturas canônicas codificadas em Norito. Cada um implementa `iroha_version::Version` para prefixar sua carga útil com a versão ABI atual (atualmente `1`) quando codificado via `EncodeVersioned`.

## Notas de revisão/atualizações potenciais

- Consultar DSL: considere documentar um subconjunto estável voltado para o usuário e exemplos de filtros/seletores comuns.
- Famílias de instruções: expanda os documentos públicos listando as variantes ISI integradas expostas por `mint_burn`, `register`, `transfer`.

---
Se alguma parte precisar de mais profundidade (por exemplo, catálogo ISI completo, lista completa de registros de consultas ou campos de cabeçalho de bloco), avise-me e estenderei essas seções de acordo.