---
lang: es
direction: ltr
source: docs/portal/docs/reference/norito-codec.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Referencia del códec Norito

Norito es la capa de serialización canónica de Iroha. Cada mensaje on-wire, payload en disco y API entre componentes usa Norito para que los nodos concuerden en bytes idénticos incluso cuando ejecutan hardware distinto. Esta pagina resume las piezas moviles y apunta a la especificacion completa en `norito.md`.

## Diseño base

| Componente | propuesta | Fuente |
| --- | --- | --- |
| **Encabezado** | Encapsula las cargas útiles con magic/version/schema hash, CRC64, longitud y etiqueta de compresión; v1 requiere `VERSION_MINOR = 0x00` y valida los flags del encabezado contra la mascara soportada (por defecto `0x00`). | `norito::header` - ver `norito.md` ("Header & Flags", raíz del repositorio) |
| **Carga útil sin encabezado** | Codificación determinista de valores usados ​​para hashing/comparacion. El transporte on-wire siempre usa encabezado; los bytes sin encabezado son solo internos. | `norito::codec::{Encode, Decode}` |
| **Compresión** | Zstd opcional (y aceleración GPU experimental) seleccionada mediante el byte de compresión del encabezado. | `norito.md`, "Negociación de compresión" |El registro de banderas de diseño (packed-struct, pack-seq, field bitset, compact lengths) vive en `norito::header::flags`. V1 usa flags `0x00` por defecto pero acepta flags explícitos dentro de la mascara soportada; los bits desconocidos se rechazan. `norito::header::Flags` se conserva para inspección interna y versiones futuras.

## Soporte de derivación

`norito_derive` ofrece derivados `Encode`, `Decode`, `IntoSchema` y ayudantes JSON. Clave de convenciones:

- Los derivados generan rutas AoS y empaquetadas; v1 usa el diseño AoS por defecto (flags `0x00`) salvo que los flags del encabezado optan por variantes empaquetadas. La implementación vive en `crates/norito_derive/src/derive_struct.rs`.
- Las funciones que afectan el diseño (`packed-struct`, `packed-seq`, `compact-len`) son opt-in vía flags del encabezado y deben codificarse/decodificarse de forma consistente entre pares.
- Los helpers JSON (`norito::json`) proporcionan JSON determinista respaldado por Norito para API abiertas. Usa `norito::json::{to_json_pretty, from_json}` - nunca `serde_json`.

## Multicodec y tablas de identificadores

Norito mantiene sus asignaciones multicodec en `norito::multicodec`. La tabla de referencia (hashes, tipos de clave, descriptores de carga útil) se mantiene en `multicodec.md` en la raíz del repositorio. Cuando se anade un nuevo identificador:1. Actualiza `norito::multicodec::registry`.
2. Extiende la tabla en `multicodec.md`.
3. Regenera los enlaces downstream (Python/Java) si consumes el mapa.

## Regenerar documentos y accesorios

Con el portal alojando por ahora un resumen en prosa, usa las fuentes Markdown upstream como fuente de verdad:

- **Especificación**: `norito.md`
- **Tabla multicódec**: `multicodec.md`
- **Puntos de referencia**: `crates/norito/benches/`
- **Pruebas de oro**: `crates/norito/tests/`

Cuando la automatización de Docusaurus entre en producción, el portal se actualizará mediante un script de sincronización (seguido en `docs/portal/scripts/`) que extrae los datos de estos archivos. Hasta entonces, mantenga esta página alineada manualmente cada vez que cambie la especificación.