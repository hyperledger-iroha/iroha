---
lang: pt
direction: ltr
source: docs/portal/docs/reference/norito-codec.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Referência do codec Norito

Norito e a camada canônica de serialização do Iroha. Toda mensagem on-wire, payload em disco e API entre componentes usa Norito para que nos concordem em bytes idênticos mesmo quando rodam em hardware diferente. Esta página resume as partes principais e aponta para a especificação completa em `norito.md`.

## Base de layout

| Componente | Proposta | Fonte |
| --- | --- | --- |
| **Cabeçalho** | Enquadra payloads com magic/version/schema hash, CRC64, length e tag de compressão; v1 requer `VERSION_MINOR = 0x00` e valida sinalizadores de cabeçalho contra uma máscara suportada (padrão `0x00`). | `norito::header` - versão `norito.md` ("Header & Flags", raiz do repositório) |
| **Cabeçalho sem carga útil** | Codificação determinística de valores usados ​​para hashing/comparação. O cabeçalho transporte on-wire sempre usa; bytes sem header são apenas internos. | `norito::codec::{Encode, Decode}` |
| **Compressão** | Zstd opcional (e aceleração GPU experimental) selecionado via o byte de compactação do cabeçalho. | `norito.md`, "Negociação de compactação" |

O registro de flags de layout (packed-struct, pack-seq, field bitset, compact lengths) fica em `norito::header::flags`. V1 usa flags `0x00` por padrão, mas aceita header flags explícitas dentro da mascara suportada; bits desconhecidos são rejeitados. `norito::header::Flags` e desligar para inspeção interna e versões futuras.

## Suporte a deriva

`norito_derive` fornece derivados `Encode`, `Decode`, `IntoSchema` e ajudantes JSON. Convenções principais:

- Deriva gerar caminhos AoS e embalado; v1 usa layout AoS por padrão (flags `0x00`) a menos que header flags optem por variantes pack. Implementação em `crates/norito_derive/src/derive_struct.rs`.
- Recursos que afetam o layout (`packed-struct`, `packed-seq`, `compact-len`) são opt-in via header flags e devem ser codificados/decodificados de forma consistente entre pares.
- Ajudantes JSON (`norito::json`) fornecem JSON determinístico suportado em Norito para APIs abertas. Use `norito::json::{to_json_pretty, from_json}` - nunca `serde_json`.

## Multicodec e tabelas de identificadores

Norito mantém suas atribuições de multicodec em `norito::multicodec`. Uma tabela de referência (hashes, tipos de chave, descritores de payload) e mantida em `multicodec.md` na raiz do repositório. Quando um novo identificador e adicionado:

1. Atualizar `norito::multicodec::registry`.
2. Estenda a tabela em `multicodec.md`.
3. Regenere as ligações downstream (Python/Java) para extraí-las do mapa.

## Regenerar documentos e fixtures

Com o portal hospedando um resumo em prosa, use as fontes Markdown upstream como fonte de verdade:

**Especificações**: `norito.md`
- **Tabela multicodec**: `multicodec.md`
- **Referências**: `crates/norito/benches/`
- **Testes de ouro**: `crates/norito/tests/`

Quando a automação de Docusaurus entra no ar, o portal será atualizado via um script de sincronização (rastreado em `docs/portal/scripts/`) que extrai os dados desses arquivos. Até lá, mantenha esta página atualizada manualmente sempre que as especificações mudarem.