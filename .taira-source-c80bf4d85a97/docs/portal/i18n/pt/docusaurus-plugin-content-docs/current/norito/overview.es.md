---
lang: pt
direction: ltr
source: docs/portal/docs/norito/overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Resumo de Norito

Norito é a capacidade de serialização binária utilizada em todo Iroha: define como codificar as estruturas de dados na rede, persistir no disco e intercambiar entre contratos e hosts. Cada caixa do espaço de trabalho depende de Norito em vez de `serde` para que pares em hardware diferente produzam bytes idênticos.

Este resumo sintetiza as peças centrais e as coloca nas referências canônicas.

## Arquitetura de um vistazo

- **Cabeça + payload** - Cada mensagem Norito começa com uma cabeça de negociação de recursos (flags, checksum) seguida da carga útil sem envolvimento. Os layouts empaquetados e a compressão são negociados através de bits da cabeça.
- **Codificação determinista** - `norito::codec::{Encode, Decode}` implementa a codificação base. O layout mismo é reutilizado em cargas úteis envoltórias em cabeçotes para que o hashing e a firma sejam mantidos deterministas.
- **Esquema + deriva** - `norito_derive` gera implementações de `Encode`, `Decode` e `IntoSchema`. As estruturas/sequências empacadas estão habilitadas por defeito e documentadas em `norito.md`.
- **Registro multicodec** - Os identificadores de hashes, tipos de chave e descritores de carga útil vivem em `norito::multicodec`. A tabela autorizada é mantida em `multicodec.md`.

##Ferramentas

| Tara | Comando/API | Notas |
| --- | --- | --- |
| Inspecionar cabecera/seções | `ivm_tool inspect <file>.to` | Mostra a versão da ABI, sinalizadores e pontos de entrada. |
| Codificar/decodificar em Rust | `norito::codec::{Encode, Decode}` | Implementado para todos os tipos principais de modelo de dados. |
| JSON de interoperabilidade | `norito::json::{to_json_pretty, from_json}` | JSON determinado respaldado pelos valores Norito. |
| Gerar documentos/especificações | `norito.md`, `multicodec.md` | Documentação fonte de verdade na raiz do repositório. |

## Fluxo de trabalho de desenvolvimento

1. **Agregar derivados** - Prefira `#[derive(Encode, Decode, IntoSchema)]` para novas estruturas de dados. Evita serializadores escritos à mão, salvo o que é absolutamente necessário.
2. **Validar layouts empaquetados** - Usa `cargo test -p norito` (e a matriz de recursos empaquetados em `scripts/run_norito_feature_matrix.sh`) para garantir que os novos layouts sejam mantidos estáveis.
3. **Regenerar documentos** - Ao alterar a codificação, atualizar `norito.md` e a tabela multicodec, depois atualizar as páginas do portal (`/reference/norito-codec` e este resumo).
4. **Manter testes Norito-first** - Os testes de integração devem usar os auxiliares JSON de Norito em vez de `serde_json` para executar as mesmas rotas de produção.

## Links rápidos

- Especificação: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Atribuições multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matriz de recursos: `scripts/run_norito_feature_matrix.sh`
- Exemplos de layout empacotado: `crates/norito/tests/`

Acompanha este resumo com o guia de início rápido (`/norito/getting-started`) para uma prática recorrida de compilar e executar o bytecode que usa cargas úteis Norito.