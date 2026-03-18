---
lang: pt
direction: ltr
source: docs/source/isi_extension_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3502fc6de75095282d44ce778b00d1b0d554773de1861d1b92f7dc573dfafa2
source_last_modified: "2026-01-03T18:07:57.214581+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Plano de extensão ISI (v1)

Esta nota confirma a ordem de prioridade para as novas Instruções Especiais Iroha e captura
invariantes não negociáveis para cada instrução antes da implementação. A ordem corresponde
primeiro o risco de segurança e operabilidade, depois o rendimento de UX.

## Pilha de prioridade

1. **RotateAccountSignatory** – Obrigatório para rotação higiênica de chaves sem migrações destrutivas.
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** – Fornece contrato determinístico
   kill switches e recuperação de armazenamento para implantações comprometidas.
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** – Estenda a paridade de metadados para ativos concretos
   saldos para que as ferramentas de observabilidade possam marcar os acervos.
4. **BatchMintAsset** / **BatchTransferAsset** – Ajudantes de distribuição determinística para manter o tamanho da carga útil
   e pressão de fallback de VM gerenciável.

## Invariantes de instrução

### SetAssetKeyValue / RemoveAssetKeyValue
- Reutilize o namespace `AssetMetadataKey` (`state.rs`) para que as chaves WSV canônicas permaneçam estáveis.
- Aplicar limites de esquema e tamanho JSON de forma idêntica aos auxiliares de metadados da conta.
- Emitir `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` com o `AssetId` afetado.
- Exigir os mesmos tokens de permissão que as edições de metadados de ativos existentes (proprietário da definição OU
  Subsídios estilo `CanModifyAssetMetadata`).
- Abortar se o registro do ativo estiver faltando (sem criação implícita).

### RotateAccountSignatory
- Troca atômica do signatário em `AccountId` preservando os metadados da conta e vinculados
  recursos (ativos, gatilhos, funções, permissões, eventos pendentes).
- Verifique se o signatário atual corresponde ao chamador (ou autoridade delegada por meio de token explícito).
- Rejeite se a nova chave pública já respalda outra conta no mesmo domínio.
- Atualize todas as chaves canônicas que incorporam o ID da conta e invalidam os caches antes do commit.
- Emita um `AccountEvent::SignatoryRotated` dedicado com chaves antigas/novas para trilhas de auditoria.
- Andaime de migração: introduza `AccountLabel` + `AccountRekeyRecord` (consulte `account::rekey`) para que
  contas existentes podem ser mapeadas para rótulos estáveis durante uma atualização contínua sem quebras de hash.

### DesativarContractInstance
- Remover ou marcar para exclusão a ligação `(namespace, contract_id)` enquanto persiste os dados de proveniência
  (quem, quando, código de razão) para solução de problemas.
- Exigir o mesmo conjunto de permissões de governança que a ativação, com ganchos de política para proibir
  desativação de namespaces do sistema principal sem aprovação elevada.
- Rejeite quando a instância já estiver inativa para manter os logs de eventos determinísticos.
- Emita um `ContractInstanceEvent::Deactivated` que os observadores downstream podem consumir.### RemoverSmartContractBytes
- Permitir a remoção do bytecode armazenado por `code_hash` somente quando não houver manifestos ou instâncias ativas
  referenciar o artefato; caso contrário, falhará com um erro descritivo.
- Registro de espelhos de porta de permissão (`CanRegisterSmartContractCode`) mais um nível de operador
  guarda (por exemplo, `CanManageSmartContractStorage`).
- Verifique se o `code_hash` fornecido corresponde ao resumo do corpo armazenado antes da exclusão para evitar
  alças obsoletas.
- Emitir `ContractCodeEvent::Removed` com hash e metadados do chamador.

### BatchMintAsset / BatchTransferAsset
- Semântica de tudo ou nada: ou toda tupla é bem-sucedida ou a instrução é abortada sem lado
  efeitos.
- Os vetores de entrada devem ser ordenados de forma determinística (sem classificação implícita) e limitados pela configuração
  (`max_batch_isi_items`).
- Emitir eventos de ativos por item para que a contabilidade downstream permaneça consistente; o contexto do lote é aditivo,
  não é um substituto.
- As verificações de permissão reutilizam a lógica de item único existente por destino (proprietário do ativo, proprietário da definição,
  ou capacidade concedida) antes da mutação do estado.
- Os conjuntos de acesso consultivos devem unir todas as chaves de leitura/gravação para manter a simultaneidade otimista correta.

## Andaime de implementação

- O modelo de dados agora carrega estruturas `SetAssetKeyValue`/`RemoveAssetKeyValue` para metadados de equilíbrio
  edições (`transparent.rs`).
- Os visitantes do executor expõem espaços reservados que bloquearão as permissões assim que a fiação do host chegar
  (`default/mod.rs`).
- Os tipos de protótipo Rekey (`account::rekey`) fornecem uma zona de destino para migrações contínuas.
- O estado mundial inclui `account_rekey_records` codificado por `AccountLabel` para que possamos preparar o rótulo →
  migrações de signatários sem tocar na codificação histórica `AccountId`.

## IVM Elaboração de Syscall

- Calços de host para `DeactivateContractInstance` / `RemoveSmartContractBytes` enviados como
  `SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) e
  `SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44), ambos consumindo TLVs Norito que espelham o
  estruturas ISI canônicas.
- Estenda `abi_syscall_list()` somente depois que os manipuladores de host espelharem os caminhos de execução `iroha_core` para manter
  Hashes ABI estáveis durante o desenvolvimento.
- Atualização do Kotodama diminuindo assim que os números do syscall se estabilizarem; adicione cobertura dourada para o expandido
  superfície ao mesmo tempo.

## Status

A ordenação e as invariantes acima estão prontas para implementação. As filiais de acompanhamento devem fazer referência
este documento ao conectar caminhos de execução e exposição ao syscall.