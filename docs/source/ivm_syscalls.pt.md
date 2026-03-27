---
lang: pt
direction: ltr
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:01:14.866000+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ABI de chamada de sistema

Este documento define os números de syscall IVM, convenções de chamada ABI de ponteiro, intervalos de números reservados e a tabela canônica de syscalls voltadas para contrato usada pela redução de Kotodama. Complementa `ivm.md` (arquitetura) e `kotodama_grammar.md` (linguagem).

Versionamento
- O conjunto de syscalls reconhecidos depende do campo `abi_version` do cabeçalho do bytecode. A primeira versão aceita apenas `abi_version = 1`; outros valores são rejeitados na admissão. Números desconhecidos para o `abi_version` ativo intercepta deterministicamente com `E_SCALL_UNKNOWN`.
- As atualizações de tempo de execução mantêm `abi_version = 1` e não expandem superfícies syscall ou pointer-ABI.
- Os custos do gás Syscall fazem parte da programação do gás versionada vinculada à versão do cabeçalho do bytecode. Consulte `ivm.md` (política de gás).

Intervalos de numeração
- `0x00..=0x1F`: núcleo/utilitário da VM (auxiliares de depuração/saída estão disponíveis em `CoreHost`; os auxiliares de desenvolvimento restantes são apenas mock-host).
- `0x20..=0x5F`: ponte ISI central Iroha (estável em ABI v1).
- `0x60..=0x7F`: ISIs de extensão controlados por recursos de protocolo (ainda parte da ABI v1 quando ativado).
- `0x80..=0xFF`: auxiliares de host/cripto e slots reservados; apenas os números presentes na lista de permissões ABI v1 são aceitos.

Ajudantes duráveis (ABI v1)
- As syscalls auxiliares de estado duráveis (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, codificação/decodificação JSON/SCHEMA) fazem parte da ABI V1 e estão incluídas na computação `abi_hash`.
- CoreHost conecta STATE_{GET,SET,DEL} ao estado de contrato inteligente durável apoiado por WSV; hosts dev/test podem persistir localmente, mas devem preservar a semântica idêntica do syscall.

Convenção de chamada Pointer-ABI (syscalls de contrato inteligente)
- Os argumentos são colocados nos registradores `r10+` como valores `u64` brutos ou como ponteiros na região INPUT para envelopes TLV Norito imutáveis (por exemplo, `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`).
- Os valores de retorno escalar são `u64` retornados do host. Os resultados do ponteiro são gravados pelo host em `r10`.

Tabela syscall canônica (subconjunto)| Hexágono | Nome | Argumentos (em `r10+`) | Devoluções | Gás (base + variável) | Notas |
|------|----------------------------|----------------------------------------------------------------------------|-------------|--------------------------------------------|-------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`, `&Name`, `&Json` | `u64=0` | `G_set_detail + bytes(val)` | Escreve um detalhe para a conta |
| 0x22 | MINT_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | Cunha `amount` do ativo para contabilizar |
| 0x23 | QUEIMAR_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | Queima `amount` da conta |
| 0x24 | TRANSFER_ASSET | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | Transferências `amount` entre contas |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | – | `u64=0` | `G_transfer` | Iniciar escopo de lote de transferência FASTPQ |
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | Liberar lote de transferência FASTPQ acumulado |
| 0x2B | TRANSFER_V1_BATCH_APPLY | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | Aplicar um lote codificado Norito em um único syscall |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | Registra um novo NFT |
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`, `&NftId`, `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | Transfere propriedade de NFT |
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | Atualiza metadados NFT |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | Queima (destrói) um NFT |
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | Consultas iteráveis ​​são executadas efêmeras; `QueryRequest::Continue` rejeitado |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS | – | `u64=count` | `G_create_nfts_for_all` | Ajudante; com recurso fechado || 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | Administrador; com recurso fechado |
| 0xA4 | GET_AUTHORITY | – (host grava o resultado) | `&AccountId`| `G_get_auth` | Host grava ponteiro para autoridade atual em `r10` |
| 0xF7 | GET_MERKLE_PATH | `addr:u64`, `out_ptr:u64`, opcional `root_out:u64` | `u64=len` | `G_mpath + len` | Grava o caminho (folha→raiz) e bytes raiz opcionais |
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`, `out_ptr:u64`, opcional `depth_cap:u64`, opcional `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`, `out_ptr:u64`, opcional `depth_cap:u64`, opcional `root_out:u64` | `u64=depth` | `G_mpath + depth` | Mesmo layout compacto para compromisso de registro |

Aplicação de gás
- CoreHost cobra gás extra para syscalls ISI usando a programação ISI nativa; As transferências em lote FASTPQ são cobradas por entrada.
- Syscalls ZK_VERIFY reutilizam a programação de gás de verificação confidencial (base + tamanho da prova).
- SMARTCONTRACT_EXECUTE_QUERY cobra base + por item + por byte; a classificação multiplica o custo por item e as compensações não classificadas adicionam uma penalidade por item.

Notas
- Todos os argumentos de ponteiro fazem referência a envelopes TLV Norito na região INPUT e são validados na primeira desreferência (`E_NORITO_INVALID` em caso de erro).
- Todas as mutações são aplicadas através do executor padrão do Iroha (através do `CoreHost`), e não diretamente pela VM.
- As constantes exatas do gás (`G_*`) são definidas pela programação do gás ativo; consulte `ivm.md`.

Erros
- `E_SCALL_UNKNOWN`: número syscall não reconhecido para o `abi_version` ativo.
- Erros de validação de entrada se propagam como interceptações de VM (por exemplo, `E_NORITO_INVALID` para TLVs malformados).

Referências cruzadas
- Arquitetura e semântica de VM: `ivm.md`
- Linguagem e mapeamento integrado: `docs/source/kotodama_grammar.md`

Nota de geração
- Uma lista completa de constantes syscall pode ser gerada a partir da fonte com:
  - `make docs-syscalls` → escreve `docs/source/ivm_syscalls_generated.md`
  - `make check-docs` → verifica se a tabela gerada está atualizada (útil em CI)
- O subconjunto acima permanece uma tabela estável e com curadoria para syscalls voltadas para contrato.

## Exemplos de TLV de administrador/função (host simulado)

Esta seção documenta as formas TLV e as cargas JSON mínimas aceitas pelo host WSV simulado para syscalls de estilo administrativo usadas em testes. Todos os argumentos de ponteiro seguem o ponteiro-ABI (envelopes TLV Norito colocados em INPUT). Os hosts de produção podem usar esquemas mais ricos; estes exemplos visam esclarecer tipos e formas básicas.- REGISTER_PEER/UNREGISTER_PEER
  - Argumentos: `r10=&Json`
  - Exemplo JSON: `{ "peer": "peer-id-or-info" }`
  - Nota CoreHost: `REGISTER_PEER` espera um objeto JSON `RegisterPeerWithPop` com bytes `peer` + `pop` (opcional `activation_at`, `expiry_at`, `hsm`); `UNREGISTER_PEER` aceita uma string de peer-id ou `{ "peer": "..." }`.

-CREATE_TRIGGER/REMOVE_TRIGGER/SET_TRIGGER_ENABLED
  -CREATE_TRIGGER:
    - Argumentos: `r10=&Json`
    - JSON mínimo: `{ "name": "t1" }` (campos adicionais ignorados pela simulação)
  - REMOVE_TRIGGER:
    - Args: `r10=&Name` (nome do gatilho)
  -SET_TRIGGER_ENABLED:
    - Args: `r10=&Name`, `r11=enabled:u64` (0 = desativado, diferente de zero = ativado)
  - Nota CoreHost: `CREATE_TRIGGER` espera uma especificação de gatilho completa (base64 Norito `Trigger` string ou
    `{ "id": "<trigger_id>", "action": ... }` com `action` como uma string base64 Norito `Action` ou
    um objeto JSON) e `SET_TRIGGER_ENABLED` alterna a chave de metadados do gatilho `__enabled` (ausente
    o padrão é habilitado).

- Funções: CREATE_ROLE/DELETE_ROLE/GRANT_ROLE/REVOKE_ROLE
  -CREATE_ROLE:
    - Args: `r10=&Name` (nome da função), `r11=&Json` (conjunto de permissões)
    - JSON aceita a chave `"perms"` ou `"permissions"`, cada uma uma matriz de strings de nomes de permissão.
    - Exemplos:
      -`{ "perms": [ "mint_asset:rose#wonder" ] }`
      -`{ "permissions": [ "read_assets:<i105-account-id>", "transfer_asset:rose#wonder" ] }`
    - Prefixos de nomes de permissão suportados na simulação:
      -`register_domain`, `register_account`, `register_asset_definition`
      -`read_assets:<account_id>`
      -`mint_asset:<asset_definition_id>`
      -`burn_asset:<asset_definition_id>`
      -`transfer_asset:<asset_definition_id>`
  - DELETE_ROLE:
    - Argumentos: `r10=&Name`
    - Falha se alguma conta ainda tiver esta função atribuída.
  - GRANT_ROLE/REVOKE_ROLE:
    - Args: `r10=&AccountId` (assunto), `r11=&Name` (nome da função)
  - Nota do CoreHost: a permissão JSON pode ser um objeto `Permission` completo (`{ "name": "...", "payload": ... }`) ou uma string (o padrão da carga útil é `null`); `GRANT_PERMISSION`/`REVOKE_PERMISSION` aceita `&Name` ou `&Json(Permission)`.

- Operações de cancelamento de registro (domínio/conta/ativo): invariantes (simulados)
  - UNREGISTER_DOMAIN (`r10=&DomainId`) falha se existirem contas ou definições de ativos no domínio.
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) falha se a conta tiver saldos diferentes de zero ou possuir NFTs.
  - UNREGISTER_ASSET (`r10=&AssetDefinitionId`) falha se existir algum saldo para o ativo.

Notas
- Esses exemplos refletem o host WSV simulado usado nos testes; hosts de nós reais podem expor esquemas administrativos mais ricos ou exigir validação adicional. As regras da ABI de ponteiro ainda se aplicam: os TLVs devem estar em INPUT, versão = 1, os IDs de tipo devem corresponder e os hashes de carga útil devem ser validados.