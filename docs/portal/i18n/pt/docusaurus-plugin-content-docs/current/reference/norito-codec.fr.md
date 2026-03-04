---
lang: pt
direction: ltr
source: docs/portal/docs/reference/norito-codec.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Referência do codec Norito

Norito é o sofá de serialização canônico de Iroha. Todas as mensagens on-wire, carga útil no disco e intercompostos de API utilizam Norito para que os nuds sejam de acordo com octetos idênticos meme quando eles se transformam em um material diferente. Esta página resume os elementos cles e o envio para a especificação completa em `norito.md`.

## Disposição de base

| Composto | Objetivo | Fonte |
| --- | --- | --- |
| **Cabeçalho** | Encapsule as cargas úteis com magic/version/schema hash, CRC64, comprimento e tag de compactação; v1 exige `VERSION_MINOR = 0x00` e valida os sinalizadores de distância contra a máscara suportada (por padrão `0x00`). | `norito::header` - ver `norito.md` ("Cabeçalho e bandeiras", racine du depot) |
| **Carga útil nu** | A codificação determina os valores utilizados para hashing/comparação. O transporte on-wire utiliza sempre um cabeçalho; os octetos não são exclusivos internos. | `norito::codec::{Encode, Decode}` |
| **Compressão** | A opção Zstd (e aceleração GPU experimental) é selecionada por meio do octeto de compactação do cabeçalho. | `norito.md`, "Negociação de compactação" |

O registro dos sinalizadores de layout (packed-struct, pack-seq, field bitset, compact lengths) está localizado em `norito::header::flags`. A v1 utiliza por padrão os sinalizadores `0x00`, mas aceita os sinalizadores explícitos na máscara suportada; os bits inconnus são rejeitados. `norito::header::Flags` é conservado para inspeção interna e versões futuras.

## Suporte para derivações

`norito_derive` fornece os derivados `Encode`, `Decode`, `IntoSchema` e os auxiliares JSON. Cláusulas de convenções:

- Les derivas generent des chemins AoS et embalado; v1 utiliza o layout AoS por padrão (sinalizadores `0x00`) salvo se os sinalizadores de entrada optarem pelas variantes empacotadas. A implementação foi encontrada em `crates/norito_derive/src/derive_struct.rs`.
- As funcionalidades que afetam o layout (`packed-struct`, `packed-seq`, `compact-len`) são ativadas por meio de sinalizadores de distância e devem ser codificadas/decodificadas de forma coerente entre pares.
- Os auxiliares JSON (`norito::json`) fornecem um JSON determinado adicionado a Norito para APIs públicas. Utilize `norito::json::{to_json_pretty, from_json}` - jamais `serde_json`.

## Multicodec e tabelas de identificação

Norito conserva suas afetações multicodec em `norito::multicodec`. A tabela de referência (hashes, tipos de arquivos, descritores de carga útil) é mantida em `multicodec.md` na linha do depósito. Lorsqu'un nouvel identifiant foi adicionado:

1. Faça um dia `norito::multicodec::registry`.
2. Mantenha a tabela em `multicodec.md`.
3. Regenere as ligações downstream (Python/Java) para usá-las no mapa.

## Regenerar documentos e fixtures

Com o portal que contém atualmente um currículo em prosa, use as fontes Markdown como fonte de verdade:

**Especificações**: `norito.md`
- **Multicodec de tabela**: `multicodec.md`
- **Referências**: `crates/norito/benches/`
- **Testes de ouro**: `crates/norito/tests/`Quando a automação Docusaurus estiver on-line, o portal será enviado novamente por meio de um script de sincronização (próximo a `docs/portal/scripts/`) que extrai os dados a partir desses arquivos. Aqui está, gardez esta página alinhada manualmente com todas as alterações nas especificações.