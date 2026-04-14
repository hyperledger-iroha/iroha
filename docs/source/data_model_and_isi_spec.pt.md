<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8d13f6d206f60d31217ed093a5bbedd7946d27b644f9b3321a577cc6065a901
source_last_modified: "2026-03-30T18:22:55.965549+00:00"
translation_last_reviewed: 2026-04-02
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
Os IDs têm formatos de string estáveis com `Display`/`FromStr` de ida e volta. As regras de nome proíbem espaços em branco e caracteres `@ # $` reservados.- `Name` — identificador textual validado. Regras: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Domínio: `{ id, logo, metadata, owned_by }`. Construtores: `NewDomain`. Código: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — endereços canônicos são produzidos via `AccountAddress`, pois I105 e Torii normalizam as entradas por meio de `AccountAddress::parse_encoded`. A análise estrita de tempo de execução aceita apenas I105 canônico. Os aliases de contas na rede usam `name@domain.dataspace` ou `name@dataspace` e resolvem para valores canônicos `AccountId`; eles não são aceitos por analisadores `AccountId` estritos. Conta: `{ id, metadata }`. Código: `crates/iroha_data_model/src/account.rs`.- Política de admissão de conta — os domínios controlam a criação implícita de contas armazenando um Norito-JSON `AccountAdmissionPolicy` na chave de metadados `iroha:account_admission_policy`. Quando a chave está ausente, o parâmetro customizado em nível de cadeia `iroha:default_account_admission_policy` fornece o padrão; quando isso também está ausente, o padrão rígido é `ImplicitReceive` (primeira versão). As tags de política `mode` (`ExplicitOnly` ou `ImplicitReceive`) mais limites opcionais por transação (padrão `16`) e por bloco, um `implicit_creation_fee` opcional (conta de gravação ou coletor), `min_initial_amounts` por definição de ativo e um opcional `default_role_on_create` (concedido após `AccountCreated`, rejeita com `DefaultRoleError` se estiver faltando). Genesis não pode aceitar; políticas desabilitadas/inválidas rejeitam instruções em estilo de recibo para contas desconhecidas com `InstructionExecutionError::AccountAdmission`. Contas implícitas carimbam metadados `iroha:created_via="implicit"` antes de `AccountCreated`; as funções padrão emitem um acompanhamento `AccountRoleGranted`, e as regras básicas do proprietário do executor permitem que a nova conta gaste seus próprios ativos/NFTs sem funções extras. Código: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.- `AssetDefinitionId` — endereço Base58 canônico sem prefixo sobre os bytes canônicos de definição de ativo. Este é o ID do ativo público. Definição: `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. Os literais `alias` devem ser `<name>#<domain>.<dataspace>` ou `<name>#<dataspace>`, com `<name>` igual ao nome da definição de ativo e são resolvidos apenas para o ID de ativo Base58 canônico. Código: `crates/iroha_data_model/src/asset/definition.rs`.
  - Os metadados de arrendamento de alias são persistidos separadamente da linha de definição de ativo armazenada. Core/Torii materializa `alias` do registro de ligação quando as definições são lidas.
  - As respostas de definição de ativo Torii expõem `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, onde `status` é `permanent`, `leased_active`, `leased_grace` ou `expired_pending_cleanup`.
  - Os seletores de alias são resolvidos de acordo com o último horário de criação do bloco confirmado. Após `grace_until_ms`, os seletores de alias param de ser resolvidos mesmo que a varredura em segundo plano ainda não tenha removido a ligação obsoleta; leituras de definição direta ainda podem relatar a ligação obsoleta como `expired_pending_cleanup`.
- `AssetId`: identificador de ativo público em formato Base58 simples e canônico. Aliases de ativos como `name#dataspace` ou `name#domain.dataspace` são resolvidos como `AssetId`. Os acervos contábeis internos também podem expor campos `asset + account + optional dataspace` divididos quando necessário, mas esse formato composto não é o `AssetId` público.
- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. Código: `crates/iroha_data_model/src/nft.rs`.- `RoleId` — `name`. Função: `{ id, permissions: BTreeSet<Permission> }` com construtor `NewRole { inner: Role, grant_to }`. Código: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Código: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — identidade do par (chave pública) e endereço. Código: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Gatilho: `{ id, action }`. Ação: `{ executable, repeats, authority, filter, metadata }`. Código: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` com inserção/remoção verificada. Código: `crates/iroha_data_model/src/metadata.rs`.
- Padrão de assinatura (camada de aplicação): os planos são entradas `AssetDefinition` com metadados `subscription_plan`; as assinaturas são registros `Nft` com metadados `subscription`; o faturamento é executado por gatilhos de tempo que fazem referência a NFTs de assinatura. Consulte `docs/source/subscriptions_api.md` e `crates/iroha_data_model/src/subscription.rs`.
- **Primitivas criptográficas** (recurso `sm`):
  - `Sm2PublicKey` / `Sm2Signature` espelha o ponto SEC1 canônico + codificação `r∥s` de largura fixa para SM2. Os construtores impõem a adesão à curva e a semântica de identificação distintiva (`DEFAULT_DISTID`), enquanto a verificação rejeita escalares malformados ou de alto alcance. Código: `crates/iroha_crypto/src/sm.rs` e `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` expõe o resumo GM/T 0004 como um novo tipo `[u8; 32]` serializável por Norito usado sempre que hashes aparecem em manifestos ou telemetria. Código: `crates/iroha_data_model/src/crypto/hash.rs`.- `Sm4Key` representa chaves SM4 de 128 bits e é compartilhado entre syscalls de host e acessórios de modelo de dados. Código: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Esses tipos acompanham as primitivas Ed25519/BLS/ML-DSA existentes e estão disponíveis para consumidores de modelo de dados (Torii, SDKs, ferramentas de gênese) assim que o recurso `sm` for habilitado.
- Armazenamentos de relacionamento derivados de espaço de dados (`space_directory_manifests`, `uaid_dataspaces`, `axt_policies`, `axt_replay_ledger`, registro de substituição de emergência de retransmissão de pista) e permissões de destino de espaço de dados (`CanPublishSpaceDirectoryManifest{dataspace: ...}` em armazenamentos de permissão de conta/função) são removidos `State::set_nexus(...)` quando os espaços para dados desaparecem do `dataspace_catalog` ativo, evitando referências de espaço para dados obsoletos após atualizações do catálogo de tempo de execução. Caches DA/relay com escopo de pista (`lane_relays`, `da_commitments`, `da_confidential_compute`, `da_pin_intents`) também são removidos quando uma pista é desativada ou reatribuída a um espaço de dados diferente para que o estado local da pista não possa vazar nas migrações de espaço de dados. Os ISIs do Space Directory (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) também validam `dataspace` em relação ao catálogo ativo e rejeitam IDs desconhecidos com `InvalidParameter`.

Características importantes: `Identifiable`, `Registered`/`Registrable` (padrão de construtor), `HasMetadata`, `IntoKeyValue`. Código: `crates/iroha_data_model/src/lib.rs`.

Eventos: Cada entidade possui eventos emitidos em mutações (criar/excluir/proprietário alterado/metadados alterados, etc.). Código: `crates/iroha_data_model/src/events/`.

---## Parâmetros (Configuração da Cadeia)
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
Tipos: `Register<T: Registered>` e `Unregister<T: Identifiable>`, com tipos de soma `RegisterBox`/`UnregisterBox` cobrindo alvos concretos.- Register Peer: insere no conjunto de peers mundiais.
  - Pré-condições: ainda não deve existir.
  - Eventos: `PeerEvent::Added`.
  - Erros: `Repetition(Register, PeerId)` se duplicado; `FindError` em pesquisas. Código: `core/.../isi/world.rs`.

- Registrar Domínio: compilações de `NewDomain` com `owned_by = authority`. Não permitido: domínio `genesis`.
  - Pré-requisitos: inexistência de domínio; não `genesis`.
  - Eventos: `DomainEvent::Created`.
  - Erros: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Código: `core/.../isi/world.rs`.

- Cadastrar conta: builds de `NewAccount`, não permitidas no domínio `genesis`; A conta `genesis` não pode ser registrada.
  - Pré-requisitos: o domínio deve existir; inexistência de conta; não no domínio da gênese.
  - Eventos: `DomainEvent::Account(AccountEvent::Created)`.
  - Erros: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Código: `core/.../isi/domain.rs`.

- Registrar AssetDefinition: builds do construtor; define `owned_by = authority`.
  - Condições prévias: inexistência de definição; domínio existe; `name` é obrigatório, não deve estar vazio após o corte e não deve conter `#`/`@`.
  - Eventos: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Erros: `Repetition(Register, AssetDefinitionId)`. Código: `core/.../isi/domain.rs`.

- Cadastrar NFT: builds do construtor; define `owned_by = authority`.
  - Pré-requisitos: inexistência de NFT; domínio existe.
  - Eventos: `DomainEvent::Nft(NftEvent::Created)`.
  - Erros: `Repetition(Register, NftId)`. Código: `core/.../isi/nft.rs`.- Função de registro: cria a partir de `NewRole { inner, grant_to }` (primeiro proprietário registrado por meio de mapeamento de função de conta), armazena `inner: Role`.
  - Condições prévias: inexistência de função.
  - Eventos: `RoleEvent::Created`.
  - Erros: `Repetition(Register, RoleId)`. Código: `core/.../isi/world.rs`.

- Registrar Trigger: armazena o trigger no trigger apropriado definido por tipo de filtro.
  - Pré-condições: Se o filtro não puder ser cunhado, `action.repeats` deve ser `Exactly(1)` (caso contrário, `MathError::Overflow`). IDs duplicados proibidos.
  - Eventos: `TriggerEvent::Created(TriggerId)`.
  - Erros: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` em falhas de conversão/validação. Código: `core/.../isi/triggers/mod.rs`.- Cancelar registro de Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger: remove o alvo; emite eventos de exclusão. Remoções em cascata adicionais:- Cancelar registro de domínio: remove a entidade do domínio mais seu estado de seletor/política de endosso; exclui definições de ativos no domínio (e estado lateral confidencial `zk_assets` codificado por essas definições), ativos dessas definições (e metadados por ativo), NFTs no domínio e projeções de alias de conta enraizadas no domínio removido. Ele também remove entradas de permissão com escopo de conta/função que fazem referência ao domínio removido ou aos recursos excluídos com ele (permissões de domínio, definição de ativo/permissões de ativo para definições removidas e permissões NFT para IDs NFT removidos). A remoção do domínio não exclui ou reescreve o `AccountId` global, seu estado de sequência tx/UAID, ativo estrangeiro ou propriedade NFT, autoridade de acionamento ou outras referências externas de auditoria/configuração que apontam para a conta sobrevivente. Guard rails: rejeita quando qualquer definição de ativo no domínio ainda é referenciada por acordo de recompra, razão de liquidação, recompensa/reivindicação de via pública, subsídio/transferência off-line, padrões de recompra de liquidação (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), votação configurada por governança/cidadania/elegibilidade do parlamento/referências de definição de ativos de recompensa viral, economia oracle configurada referências de definição de ativos de recompensa/barra/obrigação de disputa ou referências de definição de ativos de taxa/staking Nexus (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). Eventos: `DomainEvent::Deleted`, além de eventos de exclusão por item para recurso de domínio removidoces. Erros: `FindError::Domain` se ausente; `InvariantViolation` sobre conflitos de referência de definição de ativos retidos. Código: `core/.../isi/world.rs`.- Cancelar registro de conta: remove permissões, funções, contador de sequência tx, mapeamento de rótulo de conta e ligações UAID da conta; exclui ativos pertencentes à conta (e metadados por ativo); exclui NFTs de propriedade da conta; remove gatilhos cuja autoridade é aquela conta; remove entradas de permissão com escopo de conta/função que fazem referência à conta removida, permissões de destino NFT com escopo de conta/função para IDs NFT de propriedade removidos e permissões de destino de acionador com escopo de conta/função para acionadores removidos. Guard rails: rejeita se a conta ainda possui um domínio, definição de ativo, vinculação de provedor SoraFS, registro de cidadania ativa, estado de piquetagem/recompensa de via pública (incluindo chaves de reivindicação de recompensa onde a conta aparece como reclamante ou proprietário de ativo de recompensa), estado oracle ativo (incluindo entradas de provedor de histórico de feed oracle, registros de provedor de vinculação de Twitter ou referências de conta de recompensa/barra configuradas por oracle-economics), ativo Referências de conta de taxa/staking Nexus (`nexus.fees.fee_sink_account_id`, `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`; analisadas como identificadores canônicos de conta sem domínio e rejeitadas com falha fechada em literais inválidos), estado de acordo de recompra ativo, estado de livro-razão de liquidação ativo, permissão/transferência off-line ativa ou estado de revogação de veredicto off-line, off-line ativo referências de configuração de conta de garantia para definições de ativos ativos (`settlement.offline.escrow_accounts`), estado de governança ativa (proposta/aprovação de estágioals / locks / slashes / listas de conselho / parlamento, instantâneos de proposta de parlamento, registros de proponentes de atualização de tempo de execução, referências de conta de depósito / receptor de barra / pool de vírus configurados para governança, referências de remetente de telemetria de governança SoraFS via `gov.sorafs_telemetry.submitters` / `gov.sorafs_telemetry.per_provider_submitters` ou configuradas para governança SoraFS referências de proprietário-provedor via `gov.sorafs_provider_owners`), referências de conta de lista de permissão de publicação de conteúdo configurada (`content.publish_allow_accounts`), estado de remetente de garantia social ativo, estado de criador de pacote de conteúdo ativo, estado de proprietário de intenção de pino DA ativo, estado de substituição de validador de emergência de relé de pista ativo ou registro de pino SoraFS ativo registros de emissor/fichário (manifestos de pin, aliases de manifesto, ordens de replicação). Eventos: `AccountEvent::Deleted`, mais `NftEvent::Deleted` por NFT removido. Erros: `FindError::Account` se ausente; `InvariantViolation` sobre órfãos de propriedade. Código: `core/.../isi/domain.rs`.- Cancelar registro de AssetDefinition: exclui todos os ativos dessa definição e seus metadados por ativo e remove o estado lateral confidencial `zk_assets` codificado por essa definição; também remove a entrada `settlement.offline.escrow_accounts` correspondente e as entradas de permissão no escopo da conta/função que fazem referência à definição de ativo removida ou às suas instâncias de ativo. Guard rails: rejeita quando a definição ainda é referenciada por acordo de recompra, livro-razão de liquidação, recompensa/reivindicação de via pública, subsídio offline/estado de transferência, padrões de recompra de liquidação (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), votação/cidadania/elegibilidade para parlamento/referências de definição de ativos de recompensa viral configuradas para governança, configuração de economia oracle referências de definição de ativos de recompensa/barra/título de disputa ou referências de definição de ativos de taxa/staking Nexus (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). Eventos: `AssetDefinitionEvent::Deleted` e `AssetEvent::Deleted` por ativo. Erros: `FindError::AssetDefinition`, `InvariantViolation` em conflitos de referência. Código: `core/.../isi/domain.rs`.
  - Cancelar registro de NFT: remove NFT e elimina entradas de permissão com escopo de conta/função que fazem referência ao NFT removido. Eventos: `NftEvent::Deleted`. Erros: `FindError::Nft`. Código: `core/.../isi/nft.rs`.
  - Cancelar registro de função: revoga primeiro a função de todas as contas; em seguida, remove a função. Eventos: `RoleEvent::Deleted`. Erros: `FindError::Role`. Código: `core/.../isi/world.rs`.- Cancelar registro do gatilho: remove o gatilho, se presente, e remove as entradas de permissão no escopo da conta/função que fazem referência ao gatilho removido; cancelamento de registro duplicado produz `Repetition(Unregister, TriggerId)`. Eventos: `TriggerEvent::Deleted`. Código: `core/.../isi/triggers/mod.rs`.

### Menta / Queimadura
Tipos: `Mint<O, D: Identifiable>` e `Burn<O, D: Identifiable>`, embalados como `MintBox`/`BurnBox`.

- Ativo (Numérico) mint/burn: ajusta saldos e definição `total_quantity`.
  - Pré-condições: o valor `Numeric` deve satisfazer `AssetDefinition.spec()`; hortelã permitido por `mintable`:
    - `Infinitely`: sempre permitido.
    - `Once`: permitido exatamente uma vez; a primeira casa da moeda vira `mintable` para `Not` e emite `AssetDefinitionEvent::MintabilityChanged`, além de um `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` detalhado para auditabilidade.
    - `Limited(n)`: permite operações adicionais de cunhagem `n`. Cada cunhagem bem-sucedida diminui o contador; quando chega a zero, a definição muda para `Not` e emite os mesmos eventos `MintabilityChanged` acima.
    - `Not`: erro `MintabilityError::MintUnmintable`.
  - Mudanças de estado: cria ativo se estiver faltando na casa da moeda; remove a entrada de ativos se o saldo se tornar zero na queima.
  - Eventos: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (quando `Once` ou `Limited(n)` esgota seu limite).
  - Erros: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Código: `core/.../isi/asset.rs`.- Repetições de gatilho mint/burn: altera a contagem `action.repeats` para um gatilho.
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
  - Pré-requisitos: ambas as contas existem; existe definição; a fonte deve atualmente possuí-lo; a autoridade deve ser a conta de origem, o proprietário do domínio de origem ou o proprietário do domínio de definição de ativo.
  - Eventos: `AssetDefinitionEvent::OwnerChanged`.
  - Erros: `FindError::Account/AssetDefinition`. Código: `core/.../isi/account.rs`.- Propriedade de NFT: altera `Nft.owned_by` para conta de destino.
  - Pré-requisitos: ambas as contas existem; NFT existe; a fonte deve atualmente possuí-lo; a autoridade deve ser conta de origem, proprietário do domínio de origem, proprietário do domínio NFT ou possuir `CanTransferNft` para esse NFT.
  - Eventos: `NftEvent::OwnerChanged`.
  - Erros: `FindError::Account/Nft`, `InvariantViolation` se a fonte não possuir o NFT. Código: `core/.../isi/nft.rs`.

### Metadados: definir/remover valor-chave
Tipos: `SetKeyValue<T>` e `RemoveKeyValue<T>` com `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. Enums em caixa fornecidos.

- Conjunto: insere ou substitui `Metadata[key] = Json(value)`.
- Remover: remove a chave; erro se estiver faltando.
- Eventos: `<Target>Event::MetadataInserted`/`MetadataRemoved` com os valores antigos/novos.
- Erros: `FindError::<Target>` se o alvo não existir; `FindError::MetadataKey` sobre chave ausente para remoção. Código: `crates/iroha_data_model/src/isi/transparent.rs` e impls do executor por alvo.

### Permissões e funções: conceder/revogar
Tipos: `Grant<O, D>` e `Revoke<O, D>`, com enumerações em caixa para `Permission`/`Role` para/de `Account` e `Permission` para/de `Role`.- Conceder permissão para conta: adiciona `Permission`, a menos que já seja inerente. Eventos: `AccountEvent::PermissionAdded`. Erros: `Repetition(Grant, Permission)` se duplicado. Código: `core/.../isi/account.rs`.
- Revogar permissão da conta: remove, se presente. Eventos: `AccountEvent::PermissionRemoved`. Erros: `FindError::Permission` se ausente. Código: `core/.../isi/account.rs`.
- Conceder função à conta: insere o mapeamento `(account, role)` se estiver ausente. Eventos: `AccountEvent::RoleGranted`. Erros: `Repetition(Grant, RoleId)`. Código: `core/.../isi/account.rs`.
- Revogar função da conta: remove o mapeamento, se presente. Eventos: `AccountEvent::RoleRevoked`. Erros: `FindError::Role` se ausente. Código: `core/.../isi/account.rs`.
- Conceder permissão à função: reconstrói a função com permissão adicionada. Eventos: `RoleEvent::PermissionAdded`. Erros: `Repetition(Grant, Permission)`. Código: `core/.../isi/world.rs`.
- Revogar permissão da função: reconstrói a função sem essa permissão. Eventos: `RoleEvent::PermissionRemoved`. Erros: `FindError::Permission` se ausente. Código: `core/.../isi/world.rs`.### Gatilhos: Executar
Tipo: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Comportamento: enfileira um `ExecuteTriggerEvent { trigger_id, authority, args }` para o subsistema de disparo. A execução manual é permitida apenas para triggers por chamada (filtro `ExecuteTrigger`); o filtro deve corresponder e o chamador deve ser a autoridade de ação do acionador ou manter `CanExecuteTrigger` para essa autoridade. Quando um executor fornecido pelo usuário está ativo, a execução do gatilho é validada pelo executor de tempo de execução e consome o orçamento de combustível do executor da transação (base `executor.fuel` mais metadados opcionais `additional_fuel`).
- Erros: `FindError::Trigger` se não registrado; `InvariantViolation` se chamado por não autoridade. Código: `core/.../isi/triggers/mod.rs` (e testes em `core/.../smartcontracts/isi/mod.rs`).

### Atualização e registro
- `Upgrade { executor }`: migra o executor usando o bytecode `Executor` fornecido, atualiza o executor e seu modelo de dados, emite `ExecutorEvent::Upgraded`. Erros: agrupados como `InvalidParameterError::SmartContract` em caso de falha na migração. Código: `core/.../isi/world.rs`.
- `Log { level, msg }`: emite um log do nó com o nível determinado; nenhuma mudança de estado. Código: `core/.../isi/world.rs`.

### Modelo de erro
Envelope comum: `InstructionExecutionError` com variantes para erros de avaliação, falhas de consulta, conversões, entidade não encontrada, repetição, mintabilidade, matemática, parâmetro inválido e violação invariante. Enumerações e auxiliares estão em `crates/iroha_data_model/src/isi/mod.rs` em `pub mod error`.

---## Transações e executáveis
- `Executable`: `Instructions(ConstVec<InstructionBox>)` ou `Ivm(IvmBytecode)`; bytecode serializa como base64. Código: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: constrói, assina e empacota um executável com metadados, `chain_id`, `authority`, `creation_time_ms`, `ttl_ms` opcional e `nonce`. Código: `crates/iroha_data_model/src/transaction/`.
- Em tempo de execução, `iroha_core` executa lotes `InstructionBox` por meio de `Execute for InstructionBox`, fazendo downcast para o `*Box` apropriado ou instrução concreta. Código: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Orçamento de validação do executor de tempo de execução (executor fornecido pelo usuário): base `executor.fuel` de parâmetros mais metadados de transação opcionais `additional_fuel` (`u64`), compartilhados entre validações de instrução/gatilho dentro da transação.

---## Invariantes e Notas (de testes e guardas)
- Proteções Genesis: não é possível registrar o domínio `genesis` ou contas no domínio `genesis`; A conta `genesis` não pode ser registrada. Código/testes: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Os ativos numéricos devem satisfazer seu `NumericSpec` na cunhagem/transferência/queima; incompatibilidade de especificações produz `TypeError::AssetNumericSpec`.
- Cunhabilidade: `Once` permite uma única cunhagem e depois muda para `Not`; `Limited(n)` permite exatamente moedas `n` antes de mudar para `Not`. As tentativas de proibir a cunhagem em `Infinitely` causam `MintabilityError::ForbidMintOnMintable`, e a configuração de `Limited(0)` produz `MintabilityError::InvalidMintabilityTokens`.
- As operações de metadados são chave exatas; remover uma chave inexistente é um erro.
- Os filtros de gatilho podem não ser cunhados; então `Register<Trigger>` permite apenas repetições `Exactly(1)`.
- Aciona a execução dos portões da chave de metadados `__enabled` (bool); os padrões ausentes são ativados e os gatilhos desativados são ignorados nos caminhos de dados/hora/por chamada.
- Determinismo: toda aritmética utiliza operações verificadas; under/overflow retorna erros matemáticos digitados; saldos zero descartam entradas de ativos (sem estado oculto).

---## Exemplos práticos
- Cunhagem e transferência:
  - `Mint::asset_numeric(10, asset_id)` → adiciona 10 se permitido pela especificação/mintabilidade; eventos: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → move 5; eventos para remoção/adição.
- Atualizações de metadados:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → inserção superior; remoção via `RemoveKeyValue::account(...)`.
- Gerenciamento de funções/permissões:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` e suas contrapartes `Revoke`.
- Ciclo de vida do gatilho:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` com verificação de capacidade de cunhagem implícita no filtro; `ExecuteTrigger::new(id).with_args(&args)` deve corresponder à autoridade configurada.
  - Os gatilhos podem ser desabilitados definindo a chave de metadados `__enabled` para `false` (padrões ausentes para habilitado); alterne via `SetKeyValue::trigger` ou IVM `set_trigger_enabled` syscall.
  - O armazenamento de gatilhos é reparado durante o carregamento: ids duplicados, ids incompatíveis e gatilhos que fazem referência a bytecode ausentes são eliminados; as contagens de referência de bytecode são recalculadas.
  - Se o bytecode IVM de um gatilho estiver faltando no tempo de execução, o gatilho será removido e a execução será tratada como autônoma com resultado de falha.
  - Os gatilhos esgotados são removidos imediatamente; se uma entrada esgotada for encontrada durante a execução, ela será removida e tratada como ausente.
- Atualização de parâmetros:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` atualiza e emite `ConfigurationEvent::Changed`.CLI / Torii ID de definição de ativo + exemplos de alias:
- Registre-se com ID Base58 canônico + nome explícito + alias longo:
  -`iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#ubl.sbp`
- Registre-se com ID Base58 canônico + nome explícito + alias curto:
  -`iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#sbp`
- Mint por alias + componentes da conta:
  -`iroha ledger asset mint --definition-alias pkr#ubl.sbp --account <i105> --quantity 500`
- Resolva o alias para o id canônico Base58:
  -`POST /v1/assets/aliases/resolve` com JSON `{ "alias": "pkr#ubl.sbp" }`

Nota de migração:
- Os IDs de definição de ativos textuais `name#domain` permanecem intencionalmente sem suporte na primeira versão; use IDs Base58 canônicos ou resolva um alias pontilhado.
- Os seletores de ativos públicos usam IDs canônicos de definição de ativos Base58, além de campos de propriedade dividida (`account`, `scope` opcional). Literais `AssetId` com codificação bruta permanecem auxiliares internos e não fazem parte da superfície do seletor Torii/CLI.
- Filtros e classificações de lista/consulta de definição de ativos também aceitam `alias_binding.status`, `alias_binding.lease_expiry_ms`, `alias_binding.grace_until_ms` e `alias_binding.bound_at_ms`.

---

## Rastreabilidade (fontes selecionadas)
 - Núcleo do modelo de dados: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - Definições e registro ISI: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - Execução ISI: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - Eventos: `crates/iroha_data_model/src/events/**`.
 - Transações: `crates/iroha_data_model/src/transaction/**`.

Se você deseja que esta especificação seja expandida para uma tabela de API/comportamento renderizada ou vinculada a cada evento/erro concreto, diga a palavra e eu a estenderei.