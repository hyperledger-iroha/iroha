---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d4fc672641a8b65a9e6398e57eb495edac4189238fb894af9846eebc72edc2b1
source_last_modified: "2025-11-14T04:43:20.977040+00:00"
translation_last_reviewed: 2026-01-30
---

# Referencia del codec Norito

Norito es la capa de serializacion canonica de Iroha. Cada mensaje on-wire, payload en disco y API entre componentes usa Norito para que los nodos concuerden en bytes identicos incluso cuando ejecutan hardware distinto. Esta pagina resume las piezas moviles y apunta a la especificacion completa en `norito.md`.

## Diseno base

| Componente | Proposito | Fuente |
| --- | --- | --- |
| **Encabezado** | Encapsula los payloads con magic/version/schema hash, CRC64, longitud y etiqueta de compresion; v1 requiere `VERSION_MINOR = 0x00` y valida los flags del encabezado contra la mascara soportada (por defecto `0x00`). | `norito::header` - ver `norito.md` ("Header & Flags", raiz del repositorio) |
| **Payload sin encabezado** | Codificacion determinista de valores usada para hashing/comparacion. El transporte on-wire siempre usa encabezado; los bytes sin encabezado son solo internos. | `norito::codec::{Encode, Decode}` |
| **Compresion** | Zstd opcional (y aceleracion GPU experimental) seleccionada via el byte de compresion del encabezado. | `norito.md`, "Compression negotiation" |

El registro de flags de layout (packed-struct, packed-seq, field bitset, compact lengths) vive en `norito::header::flags`. V1 usa flags `0x00` por defecto pero acepta flags explicitos dentro de la mascara soportada; los bits desconocidos se rechazan. `norito::header::Flags` se conserva para inspeccion interna y versiones futuras.

## Soporte de derive

`norito_derive` ofrece derives `Encode`, `Decode`, `IntoSchema` y helpers JSON. Convenciones clave:

- Los derives generan rutas AoS y packed; v1 usa el layout AoS por defecto (flags `0x00`) salvo que los flags del encabezado opten por variantes packed. La implementacion vive en `crates/norito_derive/src/derive_struct.rs`.
- Las funciones que afectan el layout (`packed-struct`, `packed-seq`, `compact-len`) son opt-in via flags del encabezado y deben codificarse/decodificarse de forma consistente entre peers.
- Los helpers JSON (`norito::json`) proveen JSON determinista respaldado por Norito para APIs abiertas. Usa `norito::json::{to_json_pretty, from_json}` - nunca `serde_json`.

## Multicodec y tablas de identificadores

Norito mantiene sus asignaciones multicodec en `norito::multicodec`. La tabla de referencia (hashes, tipos de clave, descriptores de payload) se mantiene en `multicodec.md` en la raiz del repositorio. Cuando se anade un nuevo identificador:

1. Actualiza `norito::multicodec::registry`.
2. Extiende la tabla en `multicodec.md`.
3. Regenera los bindings downstream (Python/Java) si consumen el mapa.

## Regenerar docs y fixtures

Con el portal alojando por ahora un resumen en prosa, usa las fuentes Markdown upstream como fuente de verdad:

- **Spec**: `norito.md`
- **Tabla multicodec**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Golden tests**: `crates/norito/tests/`

Cuando la automatizacion de Docusaurus entre en produccion, el portal se actualizara mediante un script de sync (seguido en `docs/portal/scripts/`) que extrae los datos de estos archivos. Hasta entonces, manten esta pagina alineada manualmente cada vez que cambie la spec.
