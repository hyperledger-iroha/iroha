---
lang: pt
direction: ltr
source: docs/portal/docs/norito/overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Vista do conjunto de Norito

Norito é a caixa de serialização binária usada em todo Iroha: ele definitivamente comenta que as estruturas de dados são codificadas no arquivo, persistidas no disco e trocas entre contratos e hotéis. Cada caixa do espaço de trabalho é aplicada em Norito tanto quanto em `serde` para que pares em materiais diferentes produzam octetos idênticos.

Cet apercu resume les elementos cles et renvoie aux reference canonices.

## Arquitetura em um golpe de oeil

- **En-tete + payload** - Toda mensagem Norito começa em um en-tete de negociação de recursos (sinalizadores, soma de verificação) subsequente à carga útil bruta. Os pacotes de layouts e a compactação são negociados por meio dos bits do en-tete.
- **Codificação determinada** - `norito::codec::{Encode, Decode}` implementa a codificação nu. O layout do meme é reutilizado para roubar cargas úteis em todo o corpo para que a troca e a assinatura permaneçam determinadas.
- **Esquema + deriva** - `norito_derive` gera implementações `Encode`, `Decode` e `IntoSchema`. Os pacotes de estruturas/sequências são ativos por padrão e documentados em `norito.md`.
- **Registrar multicodec** - Identificadores para hashes, tipos de código e descritores de carga útil existentes em `norito::multicodec`. A tabela de referência é mantida em `multicodec.md`.

## Utilitários

| Tache | Comando / API | Notas |
| --- | --- | --- |
| Inspetor l'en-tete/seções | `ivm_tool inspect <file>.to` | Exiba a versão ABI, os sinalizadores e os pontos de entrada. |
| Codificador/decodificador em Rust | `norito::codec::{Encode, Decode}` | Implemente para todos os tipos principais do modelo de dados. |
| JSON de interoperabilidade | `norito::json::{to_json_pretty, from_json}` | JSON determina dados sobre valores Norito. |
| Documentos/especificações do gerador | `norito.md`, `multicodec.md` | Fonte de documentação de verdade na racine du repo. |

## Fluxo de trabalho de desenvolvimento

1. **Adicione os derivados** - Prefira `#[derive(Encode, Decode, IntoSchema)]` para as novas estruturas de dados. Evite os serializadores escritos à la main sauf necessite absolue.
2. **Valide os pacotes de layouts** - Use `cargo test -p norito` (e a matriz de pacotes de recursos em `scripts/run_norito_feature_matrix.sh`) para garantir que os novos layouts permaneçam estáveis.
3. **Regenerar os documentos** - Quando a codificação for alterada, coloque `norito.md` e a tabela multicodec no dia seguinte e, em seguida, atualize as páginas do portal (`/reference/norito-codec` e abra-as).
4. **Guarde os testes Norito-first** - Os testes de integração devem usar os auxiliares JSON Norito em vez de `serde_json` para exercitar os memes que estão na produção.

## Liens rapides

Especificação: [`norito.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Atribuições multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matriz de recursos: `scripts/run_norito_feature_matrix.sh`
- Exemplos de pacote de layout: `crates/norito/tests/`

Associe este guia de desarmamento rápido (`/norito/getting-started`) para um trecho prático de compilação e execução de bytecode usando cargas úteis Norito.