<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Referencia del codec Norito

Norito es la capa de serialización canónica de Iroha. Cada mensaje on-wire, payload en disco y API entre componentes usa Norito para que los nodos concuerden en bytes idénticos incluso cuando ejecutan hardware distinto. Esta página resume las piezas móviles y apunta a la especificación completa en `norito.md`.

## Diseño base

| Componente | Propósito | Fuente |
| --- | --- | --- |
| **Encabezado** | Encapsula los payloads con magic/version/schema hash, CRC64, longitud y etiqueta de compresión; v1 requiere `VERSION_MINOR = 0x00` y valida los flags del encabezado contra la máscara soportada (por defecto `0x00`). | `norito::header` — ver `norito.md` (“Header & Flags”, raíz del repositorio) |
| **Payload sin encabezado** | Codificación determinista de valores usada para hashing/comparación. El transporte on-wire siempre usa encabezado; los bytes sin encabezado son solo internos. | `norito::codec::{Encode, Decode}` |
| **Compresión** | Zstd opcional (y aceleración GPU experimental) seleccionada vía el byte de compresión del encabezado. | `norito.md`, “Compression negotiation” |

El registro de flags de layout (packed-struct, packed-seq, varint offsets, compact lengths) vive en `norito::header::flags`. V1 usa flags `0x00` por defecto pero acepta flags explícitos dentro de la máscara soportada; los bits desconocidos se rechazan. `norito::header::Flags` se conserva para inspección interna y versiones futuras.

## Soporte de derive

`norito_derive` ofrece derives `Encode`, `Decode`, `IntoSchema` y helpers JSON. Convenciones clave:

- Los derives generan rutas AoS y packed; v1 usa el layout AoS por defecto (flags `0x00`) salvo que los flags del encabezado opten por variantes packed. La implementación vive en `crates/norito_derive/src/derive_struct.rs`.
- Las funciones que afectan el layout (`packed-struct`, `packed-seq`, `compact-len`) son opt-in vía flags del encabezado y deben codificarse/decodificarse de forma consistente entre peers.
- Los helpers JSON (`norito::json`) proveen JSON determinista respaldado por Norito para APIs abiertas. Usa `norito::json::{to_json_pretty, from_json}` — nunca `serde_json`.

## Multicodec y tablas de identificadores

Norito mantiene sus asignaciones multicodec en `norito::multicodec`. La tabla de referencia (hashes, tipos de clave, descriptores de payload) se mantiene en `multicodec.md` en la raíz del repositorio. Cuando se añade un nuevo identificador:

1. Actualiza `norito::multicodec::registry`.
2. Extiende la tabla en `multicodec.md`.
3. Regenera los bindings downstream (Python/Java) si consumen el mapa.

## Regenerar docs y fixtures

Con el portal alojando por ahora un resumen en prosa, usa las fuentes Markdown upstream como fuente de verdad:

- **Spec**: `norito.md`
- **Tabla multicodec**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Golden tests**: `crates/norito/tests/`

Cuando la automatización de Docusaurus entre en producción, el portal se actualizará mediante un script de sync (seguido en `docs/portal/scripts/`) que extrae los datos de estos archivos. Hasta entonces, mantén esta página alineada manualmente cada vez que cambie la spec.
