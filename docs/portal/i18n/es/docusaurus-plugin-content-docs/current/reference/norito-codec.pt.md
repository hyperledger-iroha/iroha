---
lang: es
direction: ltr
source: docs/portal/docs/reference/norito-codec.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Referencia del códec Norito

Norito y una camada canónica de serialización de Iroha. Todos los mensajes en línea, carga útil en disco y API entre componentes usan Norito para que os concordem em bytes idénticos cuando rodam em hardware diferente. Esta página resume as partes principais e aponta para a especificacao complete em `norito.md`.

## Base de diseño

| Componente | propuesta | Fuente |
| --- | --- | --- |
| **Encabezado** | Cargas útiles de Enquadra con magic/version/schema hash, CRC64, longitud y etiqueta de compresión; v1 solicita `VERSION_MINOR = 0x00` y valida los indicadores de encabezado contra una máscara compatible (predeterminado `0x00`). | `norito::header` - ver `norito.md` ("Encabezado y banderas", raíz del repositorio) |
| **Encabezado sem de carga útil** | Codificación determinística de valores usados ​​para hashing/comparacao. O transporte por cable siempre encabezado de EE. UU.; bytes sin encabezado solo internos. | `norito::codec::{Encode, Decode}` |
| **Compresor** | Zstd opcional (y aceleración GPU experimental) seleccionada mediante el byte de compresión del encabezado. | `norito.md`, "Negociación de compresión" |

El registro de banderas de diseño (packed-struct, pack-seq, field bitset, compact lengths) es `norito::header::flags`. V1 usa flags `0x00` por padrao mas aceita header flags explícitas dentro de la máscara soportada; bits desconhecidos sao rejeitados. `norito::header::Flags` e mantido para inspección interna y versos futuros.## Soporta una derivación

`norito_derive` deriva de `Encode`, `Decode`, `IntoSchema` y ayudantes JSON. Convenções principais:

- Derivado de caminos AoS e empaquetados; v1 usa design AoS por padrao (flags `0x00`) a menos que header flags optem por variantes empaquetadas. Implementado en `crates/norito_derive/src/derive_struct.rs`.
- Los recursos que afetam diseño (`packed-struct`, `packed-seq`, `compact-len`) pueden registrarse a través de indicadores de encabezado y deben ser codificados/decodificados de forma consistente entre pares.
- Ayudantes JSON (`norito::json`) que necesitan JSON determinístico apoiado en Norito para API abiertas. Utilice `norito::json::{to_json_pretty, from_json}` - nunca `serde_json`.

## Multicodec y tablas de identificadores

Norito mantiene sus atributos de multicodec en `norito::multicodec`. Una tabla de referencia (hashes, tipos de chave, descripciones de carga útil) y mantida en `multicodec.md` en la raíz del repositorio. Cuando un nuevo identificador y aficionado:

1. Actualizar `norito::multicodec::registry`.
2. Estenda a tabela em `multicodec.md`.
3. Regenerar enlaces downstream (Python/Java) se consumen en el mapa.

## Regenerar documentos y accesorios

Como portal hospedando un resumen en prosa, utilice como fuente Markdown upstream como fuente de verdad:

- **Especificación**: `norito.md`
- **Tabla multicódec**: `multicodec.md`
- **Puntos de referencia**: `crates/norito/benches/`
- **Pruebas de oro**: `crates/norito/tests/`Cuando el automático de Docusaurus no entra, el portal se actualizará mediante un script de sincronización (rastreado en `docs/portal/scripts/`) que extrae los datos de estos archivos. Ate la, mantenha esta pagina alinhada manualmente siempre que a spec mudar.