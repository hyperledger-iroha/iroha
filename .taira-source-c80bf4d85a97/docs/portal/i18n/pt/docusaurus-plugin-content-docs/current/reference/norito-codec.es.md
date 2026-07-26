---
lang: pt
direction: ltr
source: docs/portal/docs/reference/norito-codec.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Referência do codec Norito

Norito é a capacidade de serialização canônica de Iroha. Cada mensagem on-wire, carga útil em disco e API entre componentes usa Norito para que os nós conquistem bytes idênticos, mesmo quando executados em hardware diferente. Esta página resume as peças móveis e segue a especificação completa em `norito.md`.

## Base Diseno

| Componente | Proposta | Fonte |
| --- | --- | --- |
| **Encabezado** | Encapsula as cargas úteis com magic/version/schema hash, CRC64, longitude e etiqueta de compressão; v1 requer `VERSION_MINOR = 0x00` e valida as bandeiras do encabezado contra a máscara suportada (por defeito `0x00`). | `norito::header` - versão `norito.md` ("Cabeçalho e sinalizadores", raiz do repositório) |
| **Carga útil sem encabezado** | Codificação determinista de valores usada para hashing/comparação. El transporte on-wire sempre usa encabezado; los bytes sin encabezados são apenas internos. | `norito::codec::{Encode, Decode}` |
| **Compressão** | Zstd opcional (e aceleração GPU experimental) selecionado através do byte de compressão do encabezado. | `norito.md`, "Negociação de compactação" |

O registro de sinalizadores de layout (packed-struct, pack-seq, field bitset, compact lengths) vive em `norito::header::flags`. V1 usa sinalizadores `0x00` por defeito, mas aceita sinalizadores explícitos dentro da máscara suportada; os bits desconhecidos são rechaçados. `norito::header::Flags` é conservado para inspeção interna e versões futuras.

## Suporte de derivação

`norito_derive` oferece derivados `Encode`, `Decode`, `IntoSchema` e auxiliares JSON. Convenções chaves:

- Los derivam rutas AoS y embalados; v1 usa o layout AoS por defeito (flags `0x00`) salvo que os flags do encabezado optem por variantes compactadas. A implementação ocorre em `crates/norito_derive/src/derive_struct.rs`.
- As funções que afetam o layout (`packed-struct`, `packed-seq`, `compact-len`) são ativadas por meio de sinalizadores do encabezado e devem ser codificadas/decodificadas de forma consistente entre pares.
- Los helpers JSON (`norito::json`) proveen JSON determinista respaldado por Norito para APIs abiertas. EUA `norito::json::{to_json_pretty, from_json}` - nunca `serde_json`.

## Multicodec e tabelas de identificadores

Norito mantém suas atribuições multicodec em `norito::multicodec`. A tabela de referência (hashes, tipos de chave, descritores de carga útil) é mantida em `multicodec.md` na raiz do repositório. Quando um novo identificador for encontrado:

1. Atualização `norito::multicodec::registry`.
2. Estenda a tabela em `multicodec.md`.
3. Regenera as ligações downstream (Python/Java) se consumir o mapa.

## Regenerar documentos e fixtures

Com o portal procurando por agora um resumo em prosa, use as fontes Markdown upstream como fonte de verdade:

**Especificações**: `norito.md`
- **Tabla multicodec**: `multicodec.md`
- **Referências**: `crates/norito/benches/`
- **Testes de ouro**: `crates/norito/tests/`Quando a automatização de Docusaurus entra na produção, o portal é atualizado por meio de um script de sincronização (seguido em `docs/portal/scripts/`) que extrai os dados desses arquivos. Então, mantenha esta página alinhada manualmente sempre que as especificações forem alteradas.