---
lang: pt
direction: ltr
source: docs/portal/docs/norito/overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Visão geral do Norito

Norito e a camada de serialização binária usada em todo o Iroha: define como estruturas de dados são codificadas na rede, persistidas em disco e trocadas entre contratos e hosts. Cada caixa no workspace depende de Norito em vez de `serde` para que peers em hardware diferentes produzam bytes idênticos.

Esta visão geral resume as partes principais e aponta para as referências canônicas.

## Arquitetura em resumo

- **Cabecalho + payload** - Cada mensagem Norito vem com um cabeço de negociação de recursos (flags, checksum) seguido do payload puro. Layouts embalados e compactados são negociados via bits do cabecalho.
- **Codificação determinística** - `norito::codec::{Encode, Decode}` implementa uma codificação base. O mesmo layout é reutilizado ao envolver payloads em cabecalhos para que hashing e assinatura sigam determinísticas.
- **Esquema + deriva** - `norito_derive` gera implementações de `Encode`, `Decode` e `IntoSchema`. Estruturas/sequências embaladas são ativadas por padrão e documentadas em `norito.md`.
- **Registro multicodec** - Identificadores de hashes, tipos de chave e descritores de payload residentes em `norito::multicodec`. A tabela de referência fica em `multicodec.md`.

##Ferramentas

| Tarefa | Comando/API | Notas |
| --- | --- | --- |
| Inspecionar cabecalho/secoes | `ivm_tool inspect <file>.to` | Mostra versão de ABI, flags e entrypoints. |
| Codificar/decodificar em Rust | `norito::codec::{Encode, Decode}` | Implementado para todos os tipos principais do modelo de dados. |
| JSON de interoperabilidade | `norito::json::{to_json_pretty, from_json}` | JSON determinístico suportado pelos valores Norito. |
| Gerar documentos/especificações | `norito.md`, `multicodec.md` | Documentação fonte de verdade na raiz do repo. |

## Fluxo de trabalho de desenvolvimento

1. **Adicionar deriva** - Prefira `#[derive(Encode, Decode, IntoSchema)]` para novas estruturas de dados. Evite serializadores feitos a mão salvo se for absolutamente necessário.
2. **Validar layouts embalados** - Utilize `cargo test -p norito` (e a matriz de recursos embalados em `scripts/run_norito_feature_matrix.sh`) para garantir que novos layouts fiquem estaveis.
3. **Regenerar docs** - Quando a codificação mudar, atualize `norito.md` e a tabela multicodec, depois atualize as páginas do portal (`/reference/norito-codec` e esta visão geral).
4. **Manter testes Norito-first** - Testes de integração devem usar os helpers JSON do Norito em vez de `serde_json` para experimentar os mesmos caminhos de produção.

## Links rápidos

- Especificação: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Atributos multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matriz de recursos: `scripts/run_norito_feature_matrix.sh`
- Exemplos de layout embaladodo: `crates/norito/tests/`

Combine esta visão geral com o guia de início rápido (`/norito/getting-started`) para um passo a passo prático de compilar e executar bytecode que usa cargas úteis Norito.