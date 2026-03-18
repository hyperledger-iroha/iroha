---
lang: pt
direction: ltr
source: docs/source/ivm_isi_kotodama_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3f40329b9968530dea38745b49f7fee4d55aeb461e515e6f97b5b5986cb27e3f
source_last_modified: "2026-01-21T10:20:35.513444+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ⇄ ISI ⇄ Modelo de dados ⇄ Kotodama — Revisão de alinhamento

Este documento audita como o conjunto de instruções da máquina virtual Iroha (IVM) e a superfície syscall são mapeados para as instruções especiais (ISI) Iroha e `iroha_data_model`, e como Kotodama é compilado nessa pilha. Identifica as lacunas atuais e propõe melhorias concretas para que as quatro camadas se encaixem de forma determinística e ergonômica.

Observação sobre o destino do bytecode: os contratos inteligentes Kotodama são compilados no bytecode da máquina virtual Iroha (IVM) (`.to`). Eles não visam “risc5”/RISC-V como uma arquitetura independente. Quaisquer codificações do tipo RISC-V mencionadas aqui fazem parte do formato de instrução mista do IVM e permanecem um detalhe de implementação.

## Escopo e Fontes
- IVM: `crates/ivm/src/{instruction.rs,ivm.rs,syscalls.rs,host.rs,mock_wsv.rs}` e `crates/ivm/docs/*`.
- Modelo ISI/dados: `crates/iroha_data_model/src/isi/*`, `crates/iroha_core/src/smartcontracts/isi/*` e documentos `docs/source/data_model_and_isi_spec.md`.
- Kotodama: `crates/kotodama_lang/src/*`, documentos em `crates/ivm/docs/*`.
- Integração central: `crates/iroha_core/src/{state.rs,executor.rs,smartcontracts/ivm/cache.rs}`.

Terminologia
- “ISI” refere-se a tipos de instruções integradas que alteram o estado mundial por meio do executor (por exemplo, RegisterAccount, Mint, Transfer).
- “Syscall” refere-se a IVM `SCALL` com um número de 8 bits que delega ao host operações de razão.

---

## Mapeamento atual (conforme implementado)

### Instruções IVM
- Aritmética, memória, fluxo de controle, criptografia, vetor e auxiliares ZK são definidos em `instruction.rs` e implementados em `ivm.rs`. Estas são independentes e determinísticas; caminhos de aceleração (SIMD/Metal/CUDA) possuem substitutos de CPU.
- O limite sistema/host é via `SCALL` (opcode 0x60). Os números estão listados em `syscalls.rs` e incluem operações mundiais (registro/cancelamento de registro de domínio/conta/ativo, mint/gravação/transferência, operações de função/permissão, gatilhos) além de auxiliares (`GET_PRIVATE_INPUT`, `COMMIT_OUTPUT`, `GET_MERKLE_PATH`, etc.).

### Camada de host
- A característica `IVMHost::syscall(number, &mut IVM)` reside em `host.rs`.
- DefaultHost implementa apenas auxiliares não contábeis (alocação, crescimento de heap, entradas/saídas, auxiliares de prova ZK, descoberta de recursos) — ele NÃO executa mutações de estado mundial.
- Existe uma demonstração `WsvHost` em `mock_wsv.rs` que mapeia um subconjunto de operações de ativos (Transferência/Mint/Burn) para um pequeno WSV na memória usando `AccountId`/`AssetDefinitionId` por meio de números inteiros ad hoc → mapas de ID em registros x10..x13.

### ISI e modelo de dados
- Os tipos e a semântica ISI integrados são implementados em `iroha_core::smartcontracts::isi::*` e documentados em `docs/source/data_model_and_isi_spec.md`.
- `InstructionBox` usa um registro com “wire IDs” estáveis ​​e codificação Norito; o despacho de execução nativa é o caminho do código atual no núcleo.### Integração principal do IVM
- `State::execute_trigger(..)` clona o `IVM` em cache, anexa um `CoreHost::with_accounts_and_args` e, em seguida, chama `load_program` + `run`.
- `CoreHost` implementa `IVMHost`: syscalls com estado são decodificados por meio do layout ponteiro-ABI TLV, mapeados para ISI integrado (`InstructionBox`) e enfileirados. Assim que a VM retorna, o host entrega esses ISI ao executor regular para que permissões, invariantes, eventos e telemetria permaneçam idênticos à execução nativa. Syscalls auxiliares que não tocam no WSV ainda delegam para `DefaultHost`.
- `executor.rs` continua a executar ISI integrado nativamente; migrar o próprio executor do validador para IVM continua sendo um trabalho futuro.

### Kotodama → IVM
- Existem peças de frontend (lexer/parser/minimal semântica/IR/regalloc).
- Codegen (`kotodama::compiler`) emite um subconjunto de operações IVM e usa `SCALL` para operações de ativos:
  - `MintAsset` → definir x10=conta, x11=ativo, x12=&NoritoBytes(Numeric); `SCALL SYSCALL_MINT_ASSET`.
  - `BurnAsset`/`TransferAsset` semelhante (quantidade passada como ponteiro NoritoBytes(Numeric)).
- Demonstrações `koto_*_demo.rs` mostram o uso de `WsvHost` com índices inteiros mapeados para IDs para testes rápidos.

---

## Lacunas e incompatibilidades

1) Cobertura e paridade do host principal
- Status: `CoreHost` agora existe no núcleo e traduz muitas chamadas de sistema do razão em ISI que são executadas por meio do caminho padrão. A cobertura ainda está incompleta (por exemplo, algumas funções/permissões/syscalls de gatilho são stubs) e testes de paridade são necessários para garantir que o ISI enfileirado produza os mesmos estados/eventos que a execução nativa.

2) Superfície Syscall vs. Nomenclatura e cobertura ISI/Modelo de Dados
- NFTs: syscalls agora expõem nomes canônicos `SYSCALL_NFT_*` alinhados com `iroha_data_model::nft`.
- Funções/Permissões/Gatilhos: existe uma lista de syscall, mas nenhuma implementação de referência ou tabela de mapeamento vinculando cada chamada a um ISI concreto no núcleo.
- Parâmetros/semântica: algumas syscalls não especificam codificação de parâmetros (IDs digitados vs. ponteiros) ou semântica de gás; A semântica do ISI é bem definida.

3) ABI para passar dados digitados através do limite VM/host
- Os TLVs Pointer-ABI agora são decodificados em `CoreHost` (`decode_tlv_typed`), fornecendo um caminho determinístico para IDs, metadados e cargas JSON. Resta trabalhar para garantir que cada syscall documente os tipos de ponteiro esperados e que Kotodama emita os TLVs corretos (incluindo tratamento de erros quando a política rejeita um tipo).

4) Consistência de mapeamento de gases e erros
- Os códigos de operação IVM cobram gás por operação; CoreHost agora retorna gás extra para syscalls ISI usando a programação de gás nativa (incluindo transferências de lote e a ponte ISI do fornecedor), e ZK verifica se syscalls reutilizam a programação de gás confidencial. DefaultHost ainda mantém custos mínimos para cobertura de teste.
- As superfícies de erro diferem: IVM retorna `VMError::{OutOfGas,PermissionDenied,...}`; ISI retorna categorias `InstructionExecutionError` (`Find`, `Repetition`, `InvariantViolation`, `Math`, `Type`, `Mintability`, `InvalidParameter`).5) Determinismo entre caminhos de aceleração
- IVM vector/CUDA/Metal possuem fallbacks de CPU, mas algumas operações permanecem reservadas (`SETVL`, PARBEGIN/PAREND) e ainda não fazem parte do núcleo determinístico.
- As árvores Merkle diferem entre IVM e o nó (`ivm::merkle_tree` vs `iroha_crypto::MerkleTree`) — um item de unificação já aparece em `roadmap.md`.

6) Superfície de linguagem Kotodama vs. semântica de razão pretendida
- O compilador emite um pequeno subconjunto; a maioria dos recursos de linguagem (estado/estruturas, gatilhos, permissões, parâmetros/retornos digitados) ainda não estão conectados ao modelo Host/ISI.
- Nenhuma digitação de capacidade/efeito para garantir que as syscalls sejam legais para a autoridade.

---

## Recomendações (etapas concretas)

### A. Implementar um host IVM de produção no núcleo
- Adicionar módulo `iroha_core::smartcontracts::ivm::host` implementando `ivm::host::IVMHost`.
- Para cada syscall em `ivm::syscalls`:
  - Decodifique argumentos por meio de uma ABI canônica (consulte B.), construa o ISI integrado correspondente ou chame a mesma lógica principal diretamente, execute-a em `StateTransaction` e mapeie erros deterministicamente de volta para um código de retorno IVM.
  - Carregue o gás de forma determinística usando uma tabela por syscall definida no núcleo (e exposta a IVM via `SYSCALL_GET_PARAMETER`, se necessário no futuro). Inicialmente, retorne gás extra fixo do host para cada chamada.
- Encadeie `authority: &AccountId` e `&mut StateTransaction` no host para que as verificações de permissão e os eventos sejam idênticos ao ISI nativo.
- Atualize `State::execute_trigger(ExecutableRef::Ivm)` para anexar este host antes de `vm.run()` e retornar a mesma semântica `ExecutionStep` que ISI (os eventos já são emitidos no núcleo; o comportamento consistente deve ser validado).

### B. Definir uma ABI determinística de VM/host para valores digitados
- Use Norito no lado da VM para argumentos estruturados:
  - Passe ponteiros (em x10..x13, etc.) para regiões de memória da VM contendo valores codificados em Norito para tipos como `AccountId`, `AssetDefinitionId`, `Numeric`, `Metadata`.
  - Host lê bytes através de auxiliares de memória `IVM` e decodifica com Norito (`iroha_data_model` já deriva `Encode/Decode`).
- Adicione auxiliares mínimos no codegen Kotodama para serializar IDs literais em pools de código/constantes ou para preparar quadros de chamada na memória.
- Os valores são `Numeric` e são passados ​​como ponteiros NoritoBytes; outros tipos complexos também passam por ponteiro.
- Documente isso em `crates/ivm/docs/calling_convention.md` e adicione exemplos.### C. Alinhar nomenclatura e cobertura de syscall com ISI/modelo de dados
- Renomeie syscalls relacionados a NFT para maior clareza: os nomes canônicos agora seguem o padrão `SYSCALL_NFT_*` (`SYSCALL_NFT_MINT_ASSET`, `SYSCALL_NFT_SET_METADATA`, etc.).
- Publicar uma tabela de mapeamento (doc + comentários de código) de cada syscall para a semântica principal do ISI, incluindo:
  - Parâmetros (registros versus ponteiros), pré-condições esperadas, eventos e mapeamentos de erros.
  - Taxas de gás.
- Certifique-se de que haja um syscall para cada ISI integrado que deve ser invocável a partir de Kotodama (domínios, contas, ativos, funções/permissões, gatilhos, parâmetros). Se um ISI precisar permanecer privilegiado, documente-o e aplique-o por meio de verificações de permissão no host.

### D. Unificar erros e gases
- Adicione uma camada de tradução no host: mapeie `InstructionExecutionError::{Find,Repetition,InvariantViolation,Math,Type,Mintability,InvalidParameter}` para códigos `VMError` específicos ou uma convenção de resultado estendida (por exemplo, defina `x10=0/1` e use um `VMError::HostRejected { code }` bem definido).
- Introduzir uma tabela de gases no core para syscalls; espelhe-o nos documentos IVM; garantir que os custos sejam previsíveis em termos de tamanho de entrada e independentes de plataforma.

### E. Determinismo e primitivas compartilhadas
- Unificação completa da árvore Merkle (ver roteiro) e remoção/alias `ivm::merkle_tree` para `iroha_crypto` com folhas e provas idênticas.
- Manter `SETVL`/PARBEGIN/PAREND` reservado até que verificações de determinismo de ponta a ponta e uma estratégia de escalonador determinístico estejam em vigor; documento que IVM ignora essas dicas hoje.
- Garantir que os caminhos de aceleração produzam saídas idênticas byte por byte; quando não for viável, proteja os recursos com um teste que garanta a equivalência de fallback da CPU.

### F. Fiação do compilador Kotodama
- Estender o codegen para a ABI canônica (B.) para IDs e parâmetros complexos; pare de usar mapas de demonstração de número inteiro→ID.
- Adicione mapeamento interno diretamente às syscalls ISI além dos ativos (domínios/contas/funções/permissões/gatilhos) com nomes claros.
- Adicione verificações de capacidade em tempo de compilação e anotações `permission(...)` opcionais; fallback para erros de host de tempo de execução quando a prova estática não é possível.
- Adicione testes de unidade em `crates/ivm/tests/kotodama.rs` que compilam e executam pequenos contratos de ponta a ponta usando um host de teste que decodifica argumentos Norito e modifica um WSV temporário.

### G. Documentação e ergonomia do desenvolvedor
- Atualize `docs/source/data_model_and_isi_spec.md` com a tabela de mapeamento syscall e notas ABI.
- Adicione um novo documento “IVM Host Integration Guide” em `crates/ivm/docs/` descrevendo como implementar um `IVMHost` sobre `StateTransaction` real.
- Esclareça em `README.md` e nos documentos da caixa que Kotodama tem como alvo o bytecode IVM `.to` e que syscalls são a ponte para o estado mundial.

---

## Tabela de mapeamento sugerida (rascunho inicial)

Subconjunto representativo — finalize e expanda durante a implementação do host.- SYSCALL_REGISTER_DOMAIN(id: ptr DomainId) → Registro ISI
- SYSCALL_REGISTER_ACCOUNT(id: ptr AccountId) → Registro ISI
- SYSCALL_REGISTER_ASSET(id: ptr AssetDefinitionId, mintable: u8) → Registro ISI
- SYSCALL_MINT_ASSET(conta: ptr AccountId, ativo: ptr AssetDefinitionId, valor: ptr NoritoBytes(Numeric)) → ISI Mint
- SYSCALL_BURN_ASSET(conta: ptr AccountId, ativo: ptr AssetDefinitionId, quantidade: ptr NoritoBytes(Numeric)) → ISI Burn
- SYSCALL_TRANSFER_ASSET(de: ptr AccountId, para: ptr AccountId, ativo: ptr AssetDefinitionId, valor: ptr NoritoBytes(Numeric)) → Transferência ISI
- SYSCALL_TRANSFER_V1_BATCH_BEGIN() / SYSCALL_TRANSFER_V1_BATCH_END() → ISI TransferAssetBatch (abrir/fechar o escopo; entradas individuais são baixadas via `transfer_asset`)
- SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes) → Enviar um lote pré-codificado quando os contratos já serializaram as entradas fora da cadeia
- SYSCALL_NFT_MINT_ASSET(id: ptr NftId, proprietário: ptr AccountId) → Registro ISI
- SYSCALL_NFT_TRANSFER_ASSET(de: ptr AccountId, para: ptr AccountId, id: ptr NftId) → Transferência ISI
- SYSCALL_NFT_SET_METADATA(id: ptr NftId, conteúdo: ptr Metadados) → ISI SetKeyValue
- SYSCALL_NFT_BURN_ASSET(id: ptr NftId) → Cancelar registro ISI
- SYSCALL_CREATE_ROLE(id: ptr RoleId, função: ptr Role) → Registro ISI
- SYSCALL_GRANT_ROLE(conta: ptr AccountId, função: ptr RoleId) → ISI Grant
- SYSCALL_REVOKE_ROLE(conta: ptr AccountId, função: ptr RoleId) → ISI Revoke
- SYSCALL_SET_PARAMETER (param: parâmetro ptr) → ISI SetParameter

Notas
- “ptr T” significa um ponteiro em um registro para bytes codificados em Norito para T, armazenados na memória da VM; o host o decodifica no tipo `iroha_data_model` correspondente.
- Convenção de retorno: conjuntos de sucesso `x10=1`; a falha define `x10=0` e pode gerar `VMError::HostRejected` para erros fatais.

---

## Riscos e plano de implementação
- Comece conectando o host para um conjunto restrito (Ativos + Contas) e adicione testes focados.
- Manter a execução nativa do ISI como caminho oficial enquanto a semântica do host amadurece; execute ambos os caminhos em um “modo sombra” em testes para afirmar efeitos finais e eventos idênticos.
- Assim que a paridade for validada, habilite o host IVM para gatilhos IVM em produção; mais tarde, considere rotear transações regulares por meio de IVM também.

---

## Trabalho Excelente
- Finalize os auxiliares Kotodama que passam ponteiros codificados Norito (`crates/ivm/src/kotodama_std.rs`) e os exiba por meio da CLI do compilador.
- Publique a tabela de gás syscall (incluindo syscalls auxiliares) e mantenha a aplicação/testes do CoreHost alinhados com ela.
- ✅ Adicionados fixtures Norito de ida e volta cobrindo a ABI de argumento de ponteiro; consulte `crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs` para cobertura de manifesto e ponteiro NFT mantida em CI.