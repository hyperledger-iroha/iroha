---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 608cf57a527eb97661eacbc500592b8198173c4ec10accf288758dc88b3d9001
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: pt
direction: ltr
source: docs/portal/docs/norito/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Visao geral do Norito

Norito e a camada de serializacao binaria usada em todo o Iroha: define como estruturas de dados sao codificadas na rede, persistidas em disco e trocadas entre contratos e hosts. Cada crate no workspace depende de Norito em vez de `serde` para que peers em hardware diferente produzam bytes identicos.

Esta visao geral resume as partes principais e aponta para as referencias canonicas.

## Arquitetura em resumo

- **Cabecalho + payload** - Cada mensagem Norito comeca com um cabecalho de negociacao de features (flags, checksum) seguido do payload puro. Layouts empacotados e compressao sao negociados via bits do cabecalho.
- **Codificacao deterministica** - `norito::codec::{Encode, Decode}` implementa a codificacao base. O mesmo layout e reutilizado ao envolver payloads em cabecalhos para que hashing e assinatura sigam deterministicas.
- **Schema + derives** - `norito_derive` gera implementacoes de `Encode`, `Decode` e `IntoSchema`. Structs/sequences empacotadas sao ativadas por padrao e documentadas em `norito.md`.
- **Registro multicodec** - Identificadores de hashes, tipos de chave e descritores de payload vivem em `norito::multicodec`. A tabela de referencia fica em `multicodec.md`.

## Ferramentas

| Tarefa | Comando / API | Notas |
| --- | --- | --- |
| Inspecionar cabecalho/secoes | `ivm_tool inspect <file>.to` | Mostra versao de ABI, flags e entrypoints. |
| Codificar/decodificar em Rust | `norito::codec::{Encode, Decode}` | Implementado para todos os tipos principais do data model. |
| Interop JSON | `norito::json::{to_json_pretty, from_json}` | JSON deterministico apoiado por valores Norito. |
| Gerar docs/especificacoes | `norito.md`, `multicodec.md` | Documentacao fonte de verdade na raiz do repo. |

## Fluxo de trabalho de desenvolvimento

1. **Adicionar derives** - Prefira `#[derive(Encode, Decode, IntoSchema)]` para novas estruturas de dados. Evite serializadores feitos a mao salvo se for absolutamente necessario.
2. **Validar layouts empacotados** - Use `cargo test -p norito` (e a matriz de features empacotadas em `scripts/run_norito_feature_matrix.sh`) para garantir que novos layouts fiquem estaveis.
3. **Regenerar docs** - Quando a codificacao mudar, atualize `norito.md` e a tabela multicodec, depois atualize as paginas do portal (`/reference/norito-codec` e esta visao geral).
4. **Manter testes Norito-first** - Testes de integracao devem usar os helpers JSON do Norito em vez de `serde_json` para exercitar os mesmos caminhos da producao.

## Links rapidos

- Especificacao: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Atribuicoes multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matriz de features: `scripts/run_norito_feature_matrix.sh`
- Exemplos de layout empacotado: `crates/norito/tests/`

Combine esta visao geral com o guia de inicio rapido (`/norito/getting-started`) para um passo a passo pratico de compilar e executar bytecode que usa payloads Norito.
