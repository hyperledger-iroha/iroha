---
lang: pt
direction: ltr
source: docs/portal/docs/reference/norito-codec.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Справочник кодека Norito

Norito — número canônico da série Iroha. A transferência on-wire, carga útil no disco e API móvel usam Norito, que são usados ​​para obter suporte идентичные байты даже при разном оборудовании. Esta página irá configurar os elementos de controle e usá-los de acordo com a especificação especificada em `norito.md`.

##Composição de Bazovaia

| Componente | Atualizado | Estocado |
| --- | --- | --- |
| **Cabeçalho** | Обрамляет payloads magic/version/schema hash, CRC64, длиной e тегом сжатия; v1 usa `VERSION_MINOR = 0x00` e testa sinalizadores de cabeçalho usando a máscara (por meio de `0x00`). | `norito::header` — cm. `norito.md` ("Cabeçalho e sinalizadores", repositório principal) |
| **Carga útil nua** | Determinar a codificação de código para hashing/transferência. Cabeçalho de transporte on-wire; bare байты — только внутренние. | `norito::codec::{Encode, Decode}` |
| **Compressão** | O Zstd opcional (e o uso GPU padrão), é inserido no cabeçalho. | `norito.md`, “Negociação de compactação” |

O registro de sinalizadores de layout (packed-struct, pack-seq, field bitset, compact lengths) está localizado em `norito::header::flags`. V1 para usar sinalizadores `0x00`, não принимает явные sinalizadores na máscara de segurança; неизвестные биты отклоняются. `norito::header::Flags` é uma garantia para inspeção externa e versão correta.

## Поддержка deriva

`norito_derive` fornece derivar `Encode`, `Decode`, `IntoSchema` e auxiliares JSON. Classificação de classe:

- Derive caminhos de código gerados como AoS e compactados; v1 para usar o layout AoS (sinalizadores `0x00`), exceto os sinalizadores de cabeçalho não exibem variantes compactadas. Realize a verificação em `crates/norito_derive/src/derive_struct.rs`.
- Funções, configuração de layout (`packed-struct`, `packed-seq`, `compact-len`), ativação de sinalizadores de cabeçalho e opções de ativação кодироваться/декодироваться согласованно между pares.
- Ajudantes JSON (`norito::json`) fornecem o JSON suportado por Norito para API pública. Utilize `norito::json::{to_json_pretty, from_json}` — não use `serde_json`.

## Multicodec e tabelas identificadas

Norito transfere multicodec para `norito::multicodec`. A tabela de referência (hashes, tipos de classe, descrição de carga útil) pode ser encontrada em `multicodec.md` no repositório principal. Para obter um novo identificador:

1. Abra `norito::multicodec::registry`.
2. Insira a tabela em `multicodec.md`.
3. Transforme as ligações downstream (Python/Java), apenas usando este cartão.

## Documentos e luminárias de gerenciamento

Quando o portal cria uma configuração de processo de configuração, use o Markdown upstream como essas opções:

**Especificações**: `norito.md`
- **Tabela multicodec**: `multicodec.md`
- **Referências**: `crates/norito/benches/`
- **Testes de ouro**: `crates/norito/tests/`Quando a automação Docusaurus foi ativada, o portal ativou o script de sincronização (definido em `docs/portal/scripts/`), который извлекает данные из этих файлов. Isso deve ser feito para que você possa obter a sincronização de acordo com as especificações específicas.