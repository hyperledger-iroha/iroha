---
lang: pt
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:33:51.650362+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 Modelo de dados e ISI – especificações derivadas da implementação

Esta especificação passa por engenharia reversa a partir da implementação atual em `iroha_data_model` e `iroha_core` para auxiliar na revisão do projeto. Caminhos em crases apontam para o código oficial.

## Escopo
- Define entidades canônicas (domínios, contas, ativos, NFTs, funções, permissões, pares, gatilhos) e seus identificadores.
- Descreve instruções de mudança de estado (ISI): tipos, parâmetros, pré-condições, transições de estado, eventos emitidos e condições de erro.
- Resume o gerenciamento de parâmetros, transações e serialização de instruções.

Determinismo: Todas as semânticas de instrução são transições de estado puro sem comportamento dependente de hardware. A serialização usa Norito; O bytecode da VM usa o IVM e é validado no host antes da execução na cadeia.

---

## Entidades e identificadores
Os IDs têm formatos de string estáveis com `Display`/`FromStr` ida e volta. As regras de nome proíbem espaços em branco e caracteres `@ # $` reservados.- `Name` — identificador textual validado. Regras: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Domínio: `{ id, logo, metadata, owned_by }`. Construtores: `NewDomain`. Código: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — endereços canônicos são produzidos via `AccountAddress` (IH58 / `sora…` compactado / hexadecimal) e Torii normaliza entradas por meio de `AccountAddress::parse_encoded`. IH58 é o formato de conta preferido; o formulário `sora…` é o segundo melhor para UX somente Sora. A familiar cadeia `alias` (rejected legacy form) é mantida apenas como um alias de roteamento. Conta: `{ id, metadata }`. Código: `crates/iroha_data_model/src/account.rs`.
- Política de admissão de conta — os domínios controlam a criação implícita de contas armazenando um Norito-JSON `AccountAdmissionPolicy` na chave de metadados `iroha:account_admission_policy`. Quando a chave está ausente, o parâmetro customizado em nível de cadeia `iroha:default_account_admission_policy` fornece o padrão; quando isso também está ausente, o padrão rígido é `ImplicitReceive` (primeira versão). As tags de política `mode` (`ExplicitOnly` ou `ImplicitReceive`) mais limites opcionais por transação (padrão `16`) e por bloco, um `implicit_creation_fee` opcional (conta de gravação ou coletor), `min_initial_amounts` por definição de ativo e um opcional `default_role_on_create` (concedido após `AccountCreated`, rejeita com `DefaultRoleError` se estiver faltando). Genesis não pode aceitar; políticas desabilitadas/inválidas rejeitam instruções de estilo de recibo para contas desconhecidas com `InstructionExecutionError::AccountAdmission`. Contas implícitas carimbam metadados `iroha:created_via="implicit"` antes de `AccountCreated`; as funções padrão emitem um acompanhamento `AccountRoleGranted`, e as regras básicas do proprietário do executor permitem que a nova conta gaste seus próprios ativos/NFTs sem funções extras. Código: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — `asset#domain`. Definição: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. Código: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. Código: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Função: `{ id, permissions: BTreeSet<Permission> }` com construtor `NewRole { inner: Role, grant_to }`. Código: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Código: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — identidade do par (chave pública) e endereço. Código: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Gatilho: `{ id, action }`. Ação: `{ executable, repeats, authority, filter, metadata }`. Código: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` com inserção/remoção verificada. Código: `crates/iroha_data_model/src/metadata.rs`.
- Padrão de assinatura (camada de aplicação): os planos são entradas `AssetDefinition` com metadados `subscription_plan`; as assinaturas são registros `Nft` com metadados `subscription`; o faturamento é executado por gatilhos de tempo que fazem referência a NFTs de assinatura. Consulte `docs/source/subscriptions_api.md` e `crates/iroha_data_model/src/subscription.rs`.
- **Primitivas criptográficas** (recurso `sm`):- `Sm2PublicKey` / `Sm2Signature` espelha o ponto SEC1 canônico + codificação `r∥s` de largura fixa para SM2. Os construtores impõem a adesão à curva e a semântica de identificação distintiva (`DEFAULT_DISTID`), enquanto a verificação rejeita escalares malformados ou de alto alcance. Código: `crates/iroha_crypto/src/sm.rs` e `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` expõe o resumo GM/T 0004 como um novo tipo `[u8; 32]` serializável por Norito usado sempre que hashes aparecem em manifestos ou telemetria. Código: `crates/iroha_data_model/src/crypto/hash.rs`.
  - `Sm4Key` representa chaves SM4 de 128 bits e é compartilhado entre syscalls de host e acessórios de modelo de dados. Código: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Esses tipos acompanham as primitivas Ed25519/BLS/ML-DSA existentes e estão disponíveis para consumidores de modelo de dados (Torii, SDKs, ferramentas de gênese) assim que o recurso `sm` for habilitado.

Características importantes: `Identifiable`, `Registered`/`Registrable` (padrão de construtor), `HasMetadata`, `IntoKeyValue`. Código: `crates/iroha_data_model/src/lib.rs`.

Eventos: Cada entidade possui eventos emitidos em mutações (criar/excluir/proprietário alterado/metadados alterados, etc.). Código: `crates/iroha_data_model/src/events/`.

---

## Parâmetros (Configuração da Cadeia)
- Famílias: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, mais `custom: BTreeMap`.
- Enumerações únicas para diferenças: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Agregador: `Parameters`. Código: `crates/iroha_data_model/src/parameter/system.rs`.

Configuração de parâmetros (ISI): `SetParameter(Parameter)` atualiza o campo correspondente e emite `ConfigurationEvent::Changed`. Código: `crates/iroha_data_model/src/isi/transparent.rs`, executor em `crates/iroha_core/src/smartcontracts/isi/world.rs`.

---

## Serialização e registro de instruções
- Característica principal: `Instruction: Send + Sync + 'static` com `dyn_encode()`, `as_any()`, `id()` estável (o padrão é o nome do tipo de concreto).
- `InstructionBox`: invólucro `Box<dyn Instruction>`. Clone/Eq/Ord operam em `(type_id, encoded_bytes)`, portanto a igualdade é por valor.
- Norito serde para `InstructionBox` serializa como `(String wire_id, Vec<u8> payload)` (volta para `type_name` se não houver ID de fio). A desserialização usa identificadores de mapeamento global `InstructionRegistry` para construtores. O registro padrão inclui todos os ISI integrados. Código: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: Tipos, Semântica, Erros
A execução é implementada via `Execute for <Instruction>` em `iroha_core::smartcontracts::isi`. Abaixo lista os efeitos públicos, pré-condições, eventos emitidos e erros.

### Registrar/Cancelar registro
Tipos: `Register<T: Registered>` e `Unregister<T: Identifiable>`, com tipos de soma `RegisterBox`/`UnregisterBox` cobrindo alvos concretos.

- Register Peer: insere no conjunto de peers mundiais.
  - Pré-condições: ainda não deve existir.
  - Eventos: `PeerEvent::Added`.
  - Erros: `Repetition(Register, PeerId)` se duplicado; `FindError` em pesquisas. Código: `core/.../isi/world.rs`.

- Registrar Domínio: builds de `NewDomain` com `owned_by = authority`. Não permitido: domínio `genesis`.
  - Pré-requisitos: inexistência de domínio; não `genesis`.
  - Eventos: `DomainEvent::Created`.
  - Erros: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Código: `core/.../isi/world.rs`.- Cadastrar conta: builds a partir de `NewAccount`, não permitidas no domínio `genesis`; A conta `genesis` não pode ser registrada.
  - Pré-requisitos: o domínio deve existir; inexistência de conta; não no domínio da gênese.
  - Eventos: `DomainEvent::Account(AccountEvent::Created)`.
  - Erros: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Código: `core/.../isi/domain.rs`.

- Registrar AssetDefinition: builds do construtor; define `owned_by = authority`.
  - Condições prévias: inexistência de definição; domínio existe.
  - Eventos: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Erros: `Repetition(Register, AssetDefinitionId)`. Código: `core/.../isi/domain.rs`.

- Cadastrar NFT: builds do construtor; define `owned_by = authority`.
  - Pré-requisitos: inexistência de NFT; domínio existe.
  - Eventos: `DomainEvent::Nft(NftEvent::Created)`.
  - Erros: `Repetition(Register, NftId)`. Código: `core/.../isi/nft.rs`.

- Função de registro: cria a partir de `NewRole { inner, grant_to }` (primeiro proprietário registrado por meio de mapeamento de função de conta), armazena `inner: Role`.
  - Condições prévias: inexistência de função.
  - Eventos: `RoleEvent::Created`.
  - Erros: `Repetition(Register, RoleId)`. Código: `core/.../isi/world.rs`.

- Registrar Trigger: armazena o trigger no trigger apropriado definido por tipo de filtro.
  - Pré-condições: Se o filtro não puder ser cunhado, `action.repeats` deve ser `Exactly(1)` (caso contrário, `MathError::Overflow`). IDs duplicados proibidos.
  - Eventos: `TriggerEvent::Created(TriggerId)`.
  - Erros: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` em falhas de conversão/validação. Código: `core/.../isi/triggers/mod.rs`.

- Cancelar registro de Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger: remove o alvo; emite eventos de exclusão. Remoções em cascata adicionais:
  - Cancelar registro de domínio: remove todas as contas no domínio, suas funções, permissões, contadores de sequência tx, rótulos de conta e ligações UAID; exclui seus ativos (e metadados por ativo); remove todas as definições de ativos no domínio; exclui NFTs no domínio e quaisquer NFTs pertencentes às contas removidas; remove gatilhos cujo domínio de autoridade corresponde. Eventos: `DomainEvent::Deleted`, mais eventos de exclusão por item. Erros: `FindError::Domain` se estiver ausente. Código: `core/.../isi/world.rs`.
  - Cancelar registro de conta: remove permissões, funções, contador de sequência tx, mapeamento de rótulo de conta e ligações UAID da conta; exclui ativos de propriedade da conta (e metadados por ativo); exclui NFTs de propriedade da conta; remove gatilhos cuja autoridade é essa conta. Eventos: `AccountEvent::Deleted`, mais `NftEvent::Deleted` por NFT removido. Erros: `FindError::Account` se estiver ausente. Código: `core/.../isi/domain.rs`.
  - Cancelar registro de AssetDefinition: exclui todos os ativos dessa definição e seus metadados por ativo. Eventos: `AssetDefinitionEvent::Deleted` e `AssetEvent::Deleted` por ativo. Erros: `FindError::AssetDefinition`. Código: `core/.../isi/domain.rs`.
  - Cancelar registro de NFT: remove NFT. Eventos: `NftEvent::Deleted`. Erros: `FindError::Nft`. Código: `core/.../isi/nft.rs`.
  - Cancelar registro de função: revoga primeiro a função de todas as contas; em seguida, remove a função. Eventos: `RoleEvent::Deleted`. Erros: `FindError::Role`. Código: `core/.../isi/world.rs`.
  - Unregister Trigger: remove o trigger se presente; o cancelamento de registro duplicado produz `Repetition(Unregister, TriggerId)`. Eventos: `TriggerEvent::Deleted`. Código: `core/.../isi/triggers/mod.rs`.

### Menta / Queimadura
Tipos: `Mint<O, D: Identifiable>` e `Burn<O, D: Identifiable>`, embalados como `MintBox`/`BurnBox`.- Ativo (Numérico) mint/burn: ajusta saldos e definição `total_quantity`.
  - Pré-condições: o valor `Numeric` deve satisfazer `AssetDefinition.spec()`; hortelã permitido por `mintable`:
    - `Infinitely`: sempre permitido.
    - `Once`: permitido exatamente uma vez; a primeira casa da moeda vira `mintable` para `Not` e emite `AssetDefinitionEvent::MintabilityChanged`, além de um `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` detalhado para auditabilidade.
    - `Limited(n)`: permite operações adicionais de cunhagem `n`. Cada cunhagem bem-sucedida diminui o contador; quando chega a zero, a definição muda para `Not` e emite os mesmos eventos `MintabilityChanged` acima.
    - `Not`: erro `MintabilityError::MintUnmintable`.
  - Mudanças de estado: cria ativo se estiver faltando na casa da moeda; remove a entrada de ativos se o saldo se tornar zero na queima.
  - Eventos: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (quando `Once` ou `Limited(n)` esgota seu limite).
  - Erros: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Código: `core/.../isi/asset.rs`.

- Repetições de gatilho mint/burn: altera a contagem `action.repeats` para um gatilho.
  - Pré-requisitos: na hortelã, o filtro deve ser cunhado; a aritmética não deve transbordar/subfluir.
  - Eventos: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - Erros: `MathError::Overflow` em mint inválido; `FindError::Trigger` se estiver faltando. Código: `core/.../isi/triggers/mod.rs`.

### Transferência
Tipos: `Transfer<S: Identifiable, O, D: Identifiable>`, embalado como `TransferBox`.

- Ativo (Numérico): subtrair da origem `AssetId`, adicionar ao destino `AssetId` (mesma definição, conta diferente). Exclua o ativo de origem zerado.
  - Pré-condições: o ativo de origem existe; o valor satisfaz `spec`.
  - Eventos: `AssetEvent::Removed` (fonte), `AssetEvent::Added` (destino).
  - Erros: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. Código: `core/.../isi/asset.rs`.

- Propriedade do domínio: altera `Domain.owned_by` para conta de destino.
  - Pré-requisitos: ambas as contas existem; domínio existe.
  - Eventos: `DomainEvent::OwnerChanged`.
  - Erros: `FindError::Account/Domain`. Código: `core/.../isi/domain.rs`.

- Propriedade de AssetDefinition: altera `AssetDefinition.owned_by` para a conta de destino.
  - Pré-requisitos: ambas as contas existem; existe definição; a fonte deve atualmente possuí-lo.
  - Eventos: `AssetDefinitionEvent::OwnerChanged`.
  - Erros: `FindError::Account/AssetDefinition`. Código: `core/.../isi/account.rs`.

- Propriedade de NFT: altera `Nft.owned_by` para conta de destino.
  - Pré-requisitos: ambas as contas existem; NFT existe; a fonte deve atualmente possuí-lo.
  - Eventos: `NftEvent::OwnerChanged`.
  - Erros: `FindError::Account/Nft`, `InvariantViolation` se a fonte não possuir o NFT. Código: `core/.../isi/nft.rs`.

### Metadados: definir/remover valor-chave
Tipos: `SetKeyValue<T>` e `RemoveKeyValue<T>` com `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. Enums em caixa fornecidos.

- Conjunto: insere ou substitui `Metadata[key] = Json(value)`.
- Remover: remove a chave; erro se estiver faltando.
- Eventos: `<Target>Event::MetadataInserted`/`MetadataRemoved` com os valores antigos/novos.
- Erros: `FindError::<Target>` se o alvo não existir; `FindError::MetadataKey` sobre chave ausente para remoção. Código: `crates/iroha_data_model/src/isi/transparent.rs` e impls do executor por alvo.### Permissões e funções: conceder/revogar
Tipos: `Grant<O, D>` e `Revoke<O, D>`, com enumerações em caixa para `Permission`/`Role` para/de `Account` e `Permission` para/de `Role`.

- Conceder permissão para conta: adiciona `Permission`, a menos que já seja inerente. Eventos: `AccountEvent::PermissionAdded`. Erros: `Repetition(Grant, Permission)` se duplicado. Código: `core/.../isi/account.rs`.
- Revogar permissão da conta: remove, se presente. Eventos: `AccountEvent::PermissionRemoved`. Erros: `FindError::Permission` se ausente. Código: `core/.../isi/account.rs`.
- Conceder função à conta: insere o mapeamento `(account, role)` se estiver ausente. Eventos: `AccountEvent::RoleGranted`. Erros: `Repetition(Grant, RoleId)`. Código: `core/.../isi/account.rs`.
- Revogar função da conta: remove o mapeamento, se presente. Eventos: `AccountEvent::RoleRevoked`. Erros: `FindError::Role` se ausente. Código: `core/.../isi/account.rs`.
- Conceder permissão à função: reconstrói a função com permissão adicionada. Eventos: `RoleEvent::PermissionAdded`. Erros: `Repetition(Grant, Permission)`. Código: `core/.../isi/world.rs`.
- Revogar permissão da função: reconstrói a função sem essa permissão. Eventos: `RoleEvent::PermissionRemoved`. Erros: `FindError::Permission` se ausente. Código: `core/.../isi/world.rs`.

### Gatilhos: Executar
Tipo: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Comportamento: enfileira um `ExecuteTriggerEvent { trigger_id, authority, args }` para o subsistema de disparo. A execução manual é permitida apenas para triggers por chamada (filtro `ExecuteTrigger`); o filtro deve corresponder e o chamador deve ser a autoridade de ação do acionador ou manter `CanExecuteTrigger` para essa autoridade. Quando um executor fornecido pelo usuário está ativo, a execução do gatilho é validada pelo executor de tempo de execução e consome o orçamento de combustível do executor da transação (base `executor.fuel` mais metadados opcionais `additional_fuel`).
- Erros: `FindError::Trigger` se não registrado; `InvariantViolation` se chamado por não autoridade. Código: `core/.../isi/triggers/mod.rs` (e testes em `core/.../smartcontracts/isi/mod.rs`).

### Atualização e registro
- `Upgrade { executor }`: migra o executor usando o bytecode `Executor` fornecido, atualiza o executor e seu modelo de dados, emite `ExecutorEvent::Upgraded`. Erros: agrupados como `InvalidParameterError::SmartContract` em caso de falha na migração. Código: `core/.../isi/world.rs`.
- `Log { level, msg }`: emite um log do nó com o nível determinado; nenhuma mudança de estado. Código: `core/.../isi/world.rs`.

### Modelo de erro
Envelope comum: `InstructionExecutionError` com variantes para erros de avaliação, falhas de consulta, conversões, entidade não encontrada, repetição, mintabilidade, matemática, parâmetro inválido e violação invariável. Enumerações e auxiliares estão em `crates/iroha_data_model/src/isi/mod.rs` em `pub mod error`.

---## Transações e executáveis
- `Executable`: `Instructions(ConstVec<InstructionBox>)` ou `Ivm(IvmBytecode)`; bytecode serializa como base64. Código: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: constrói, assina e empacota um executável com metadados, `chain_id`, `authority`, `creation_time_ms`, `ttl_ms` opcional e `nonce`. Código: `crates/iroha_data_model/src/transaction/`.
- Em tempo de execução, `iroha_core` executa lotes `InstructionBox` por meio de `Execute for InstructionBox`, fazendo downcast para o `*Box` apropriado ou instrução concreta. Código: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Orçamento de validação do executor de tempo de execução (executor fornecido pelo usuário): base `executor.fuel` de parâmetros mais metadados de transação opcionais `additional_fuel` (`u64`), compartilhados entre validações de instrução/gatilho dentro da transação.

---

## Invariantes e Notas (de testes e guardas)
- Proteções Genesis: não é possível registrar o domínio `genesis` ou contas no domínio `genesis`; A conta `genesis` não pode ser registrada. Código/testes: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Os ativos numéricos devem satisfazer seu `NumericSpec` na cunhagem/transferência/queima; incompatibilidade de especificações produz `TypeError::AssetNumericSpec`.
- Cunhabilidade: `Once` permite uma única cunhagem e depois muda para `Not`; `Limited(n)` permite exatamente moedas `n` antes de mudar para `Not`. As tentativas de proibir a cunhagem em `Infinitely` causam `MintabilityError::ForbidMintOnMintable`, e a configuração de `Limited(0)` produz `MintabilityError::InvalidMintabilityTokens`.
- As operações de metadados são chave exatas; remover uma chave inexistente é um erro.
- Os filtros de gatilho podem não ser cunhados; então `Register<Trigger>` permite apenas repetições `Exactly(1)`.
- Acionar a chave de metadados `__enabled` (bool) para execução dos portões; os padrões ausentes são ativados e os gatilhos desativados são ignorados nos caminhos de dados/hora/por chamada.
- Determinismo: toda aritmética utiliza operações verificadas; under/overflow retorna erros matemáticos digitados; saldos zero descartam entradas de ativos (sem estado oculto).

---## Exemplos práticos
- Cunhagem e transferência:
  - `Mint::asset_numeric(10, asset_id)` → adiciona 10 se permitido pela especificação/mintabilidade; eventos: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → movimentos 5; eventos para remoção/adição.
- Atualizações de metadados:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → upsert; remoção via `RemoveKeyValue::account(...)`.
- Gerenciamento de funções/permissões:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` e suas contrapartes `Revoke`.
- Ciclo de vida do gatilho:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` com verificação de capacidade de cunhagem implícita no filtro; `ExecuteTrigger::new(id).with_args(&args)` deve corresponder à autoridade configurada.
  - Os gatilhos podem ser desabilitados definindo a chave de metadados `__enabled` para `false` (padrões ausentes para habilitado); alterne via `SetKeyValue::trigger` ou IVM `set_trigger_enabled` syscall.
  - O armazenamento de gatilhos é reparado durante o carregamento: ids duplicados, ids incompatíveis e gatilhos que fazem referência a bytecode ausentes são descartados; as contagens de referência de bytecode são recalculadas.
  - Se o bytecode IVM de um gatilho estiver faltando no tempo de execução, o gatilho será removido e a execução será tratada como autônoma com resultado de falha.
  - Os gatilhos esgotados são removidos imediatamente; se uma entrada esgotada for encontrada durante a execução, ela será removida e tratada como ausente.
- Atualização de parâmetros:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` atualiza e emite `ConfigurationEvent::Changed`.

---

## Rastreabilidade (fontes selecionadas)
 - Núcleo do modelo de dados: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - Definições e registro ISI: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - Execução ISI: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - Eventos: `crates/iroha_data_model/src/events/**`.
 - Transações: `crates/iroha_data_model/src/transaction/**`.

Se você deseja que esta especificação seja expandida para uma tabela de API/comportamento renderizada ou vinculada a cada evento/erro concreto, diga a palavra e eu a estenderei.