---
lang: es
direction: ltr
source: docs/portal/docs/reference/norito-codec.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Referencia do codec Norito

Norito e a camada canonica de serializacao do Iroha. Toda mensagem on-wire, payload em disco e API entre componentes usa Norito para que os nos concordem em bytes identicos mesmo quando rodam em hardware diferente. Esta pagina resume as partes principais e aponta para a especificacao completa em `norito.md`.

## Layout base

| Componente | Proposito | Fonte |
| --- | --- | --- |
| **Header** | Enquadra payloads com magic/version/schema hash, CRC64, length e tag de compressao; v1 requer `VERSION_MINOR = 0x00` e valida header flags contra a mascara suportada (default `0x00`). | `norito::header` - ver `norito.md` ("Header & Flags", raiz do repositorio) |
| **Payload sem header** | Codificacao deterministica de valores usada para hashing/comparacao. O transporte on-wire sempre usa header; bytes sem header sao apenas internos. | `norito::codec::{Encode, Decode}` |
| **Compressao** | Zstd opcional (e aceleracao GPU experimental) selecionada via o byte de compressao do header. | `norito.md`, "Compression negotiation" |

O registro de flags de layout (packed-struct, packed-seq, field bitset, compact lengths) fica em `norito::header::flags`. V1 usa flags `0x00` por padrao mas aceita header flags explicitas dentro da mascara suportada; bits desconhecidos sao rejeitados. `norito::header::Flags` e mantido para inspecao interna e versoes futuras.

## Suporte a derive

`norito_derive` fornece derives `Encode`, `Decode`, `IntoSchema` e helpers JSON. Convencoes principais:

- Derives geram caminhos AoS e packed; v1 usa layout AoS por padrao (flags `0x00`) a menos que header flags optem por variantes packed. Implementacao em `crates/norito_derive/src/derive_struct.rs`.
- Recursos que afetam layout (`packed-struct`, `packed-seq`, `compact-len`) sao opt-in via header flags e devem ser codificados/decodificados de forma consistente entre peers.
- JSON helpers (`norito::json`) fornecem JSON deterministico apoiado em Norito para APIs abertas. Use `norito::json::{to_json_pretty, from_json}` - nunca `serde_json`.

## Multicodec e tabelas de identificadores

Norito mantem suas atribuicoes de multicodec em `norito::multicodec`. A tabela de referencia (hashes, tipos de chave, descritores de payload) e mantida em `multicodec.md` na raiz do repositorio. Quando um novo identificador e adicionado:

1. Atualize `norito::multicodec::registry`.
2. Estenda a tabela em `multicodec.md`.
3. Regenere bindings downstream (Python/Java) se consumirem o mapa.

## Regenerar docs e fixtures

Com o portal hospedando um resumo em prosa, use as fontes Markdown upstream como fonte de verdade:

- **Spec**: `norito.md`
- **Multicodec table**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Golden tests**: `crates/norito/tests/`

Quando a automacao de Docusaurus entrar no ar, o portal sera atualizado via um script de sync (rastreado em `docs/portal/scripts/`) que extrai os dados desses arquivos. Ate la, mantenha esta pagina alinhada manualmente sempre que a spec mudar.
