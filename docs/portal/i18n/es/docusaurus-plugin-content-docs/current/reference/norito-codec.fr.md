---
lang: es
direction: ltr
source: docs/portal/docs/reference/norito-codec.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Referencia del códec Norito

Norito es el sofá de serialización canónica de Iroha. Cada mensaje en línea, carga útil en disco y API entre componentes utilizan Norito para que los nuds se ajusten a los octetos idénticos memes cuando giran en material diferente. Esta página resume los elementos y el envío según la especificación completa en `norito.md`.

## Disposición de base

| Componente | Objetivo | Fuente |
| --- | --- | --- |
| **Encabezado** | Encapsule las cargas útiles con magic/version/schema hash, CRC64, longitud y etiqueta de compresión; v1 exige `VERSION_MINOR = 0x00` y valida las banderas de tu dispositivo contra la máscara compatible (por defecto `0x00`). | `norito::header` - ver `norito.md` ("Encabezado y banderas", racine du depot) |
| **Carga útil nu** | La codificación determina los valores que se utilizan para el hash/comparación. El transporte por cable utiliza siempre un encabezado; les octetos nus sont internos únicos. | `norito::codec::{Encode, Decode}` |
| **Compresión** | Selección de opciones estándar (y aceleración experimental de GPU) a través del octeto de compresión del encabezado. | `norito.md`, "Negociación de compresión" |El registro de indicadores de diseño (estructura empaquetada, secuencia empaquetada, conjunto de bits de campo, longitudes compactas) se encuentra en `norito::header::flags`. La v1 utiliza por defecto las banderas `0x00` pero acepta las banderas explícitas en la máscara de soporte; Los bits inconnus sont rechazados. `norito::header::Flags` se conserva para la inspección interna y las versiones futuras.

## Soporte de derivaciones

`norito_derive` contiene archivos derivados `Encode`, `Decode`, `IntoSchema` y los ayudantes JSON. Convenciones cles:

- Los derivados generent des chemins AoS et envasados; v1 utiliza el diseño AoS por defecto (flags `0x00`) pero si las banderas en-tete optan por las variantes empaquetadas. La implementación se encuentra en `crates/norito_derive/src/derive_struct.rs`.
- Las funciones que afectan el diseño (`packed-struct`, `packed-seq`, `compact-len`) se activan a través de las banderas de la computadora y deben codificarse/decodificarse de manera coherente entre pares.
- Los ayudantes JSON (`norito::json`) proporcionan un JSON determinado junto con Norito para las API públicas. Utilice `norito::json::{to_json_pretty, from_json}` - jamais `serde_json`.

## Multicodec y tablas de identificadores

Norito conserva sus afectaciones multicódec en `norito::multicodec`. La tabla de referencia (hashes, tipos de elementos, descriptores de carga útil) se mantiene en `multicodec.md` en la ubicación del depósito. Lorsqu'un nouvel identifiant est ajoute:1. Mettez al día `norito::multicodec::registry`.
2. Extienda la mesa en `multicodec.md`.
3. Regenere los enlaces posteriores (Python/Java) en el mapa.

## Regenerar los documentos y accesorios

Con el portail qui heberge actuellement un curriculum vitae en prosa, utilice las fuentes Markdown como fuente de verdad:

- **Especificación**: `norito.md`
- **Tabla multicódec**: `multicodec.md`
- **Puntos de referencia**: `crates/norito/benches/`
- **Pruebas de oro**: `crates/norito/tests/`

Cuando la automatización Docusaurus será en línea, el portal será un día a través de un script de sincronización (suivi dans `docs/portal/scripts/`) que extrae los datos después de estos archivos. D'ici la, gardez esta página alineada manualmente a cada cambio de especificación.