---
lang: pt
direction: ltr
source: docs/source/error_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cba8780bcec4ebf562dc9c5725f328b0ea2d9009517efa5b5a504e2fb6be81fe
source_last_modified: "2026-01-18T05:31:56.950113+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Guia de mapeamento de erros

Última atualização: 21/08/2025

Este guia mapeia modos de falha comuns em Iroha para categorias de erro estáveis apresentadas pelo modelo de dados. Use-o para projetar testes e tornar previsível o tratamento de erros do cliente.

Princípios
- Os caminhos de instrução e consulta emitem enumerações estruturadas. Evite pânico; reportar uma categoria específica sempre que possível.
- As categorias são estáveis, as mensagens podem evoluir. Os clientes devem combinar categorias, não strings de formato livre.

Categorias
- InstructionExecutionError::Find: Entidade ausente (ativo, conta, domínio, NFT, função, gatilho, permissão, chave pública, bloco, transação). Exemplo: remover uma chave de metadados inexistente resulta em Find(MetadataKey).
- InstructionExecutionError::Repetition: Registro duplicado ou ID conflitante. Contém o tipo de instrução e o IdBox repetido.
- InstructionExecutionError::Mintability: invariante de mintabilidade violada (`Once` esgotado duas vezes, `Limited(n)` esgotado ou tentativas de desabilitar `Infinitely`). Exemplos: cunhar um ativo definido como `Once` rende duas vezes `Mintability(MintUnmintable)`; configurar `Limited(0)` produz `Mintability(InvalidMintabilityTokens)`.
- InstructionExecutionError::Math: Erros de domínio numérico (estouro, divisão por zero, valor negativo, quantidade insuficiente). Exemplo: queimar mais do que a quantidade disponível produz Math(NotEnoughQuantity).
- InstructionExecutionError::InvalidParameter: Parâmetro ou configuração de instrução inválida (por exemplo, disparo de tempo no passado). Use para cargas de contrato malformadas.
- InstructionExecutionError::Evaluate: incompatibilidade de DSL/especificação para formato ou tipos de instrução. Exemplo: especificação numérica errada para um valor de ativo produz Evaluate(Type(AssetNumericSpec(..))).
- InstructionExecutionError::InvariantViolation: Violação de uma invariante do sistema que não pode ser expressa em outras categorias. Exemplo: tentativa de remoção do último signatário.
- InstructionExecutionError::Query: Wrapping de QueryExecutionFail quando uma consulta falha durante a execução da instrução.

QueryExecutionFail
- Localizar: entidade ausente no contexto da consulta.
- Conversão: Tipo errado esperado por uma consulta.
- NotFound: cursor de consulta ao vivo ausente.
- CursorMismatch / CursorDone: Erros de protocolo do cursor.
- FetchSizeTooBig: limite imposto pelo servidor excedido.
- GasBudgetExceeded: a execução da consulta excedeu o orçamento de gás/materialização.
- InvalidSingularParameters: parâmetros não suportados para consultas singulares.
- CapacityLimit: capacidade de armazenamento de consultas em tempo real atingida.

Dicas de teste
- Prefira testes unitários próximos à origem do erro. Por exemplo, a incompatibilidade de especificações numéricas de ativos pode ser gerada em testes de modelos de dados.
- Os testes de integração devem abranger o mapeamento extremo-a-extremo para casos representativos (por exemplo, registo duplicado, falta de chave ao remover, transferência sem propriedade).
- Mantenha as asserções resilientes combinando variantes de enum em vez de substrings de mensagens.