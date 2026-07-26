---
lang: es
direction: ltr
source: docs/portal/docs/reference/norito-codec.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Referencia del códec Norito

Norito Iroha کی capa de serialización canónica ہے۔ Mensaje en línea, carga útil en disco, API de componentes cruzados Norito, configuración de nodos, hardware, bytes رہیں۔ یہ صفحہ اہم حصے خلاصہ کرتا ہے اور مکمل especificación کے لئے `norito.md` کی طرف اشارہ کرتا ہے۔

## Diseño central

| Componente | Propósito | Fuente |
| --- | --- | --- |
| **Encabezado** | magic/version/schema hash, CRC64, longitud, etiqueta de compresión y cargas útiles v1 میں `VERSION_MINOR = 0x00` ضروری ہے اور banderas de encabezado کو máscara compatible کے مقابل validar کیا جاتا ہے (predeterminado `0x00`). | `norito::header` — `norito.md` ("Encabezado y banderas", raíz del repositorio) دیکھیں |
| **Carga útil desnuda** | hash/موازنہ کیلئے codificación de valor determinista۔ Transporte por cable ہمیشہ encabezado استعمال کرتا ہے؛ bytes desnudos صرف اندرونی استعمال کیلئے ہیں۔ | `norito::codec::{Encode, Decode}` |
| **Compresión** | Zstd opcional (aceleración de GPU experimental) جو encabezado کے byte de compresión کے ذریعے منتخب ہوتی ہے۔ | `norito.md`, “Negociación de compresión” |registro de indicadores de diseño (estructura empaquetada, secuencia empaquetada, conjunto de bits de campo, longitudes compactas) `norito::header::flags` میں ہے۔ Valores predeterminados de V1 کے کرتا ہے؛ bits desconocidos رد کئے جاتے ہیں۔ `norito::header::Flags` اندرونی inspección اور مستقبل کی versiones کیلئے رکھا جاتا ہے۔

## Derivar soporte

`norito_derive` `Encode`, `Decode`, `IntoSchema` y el ayudante JSON deriva فراہم کرتا ہے۔ اہم convenciones:

- Deriva AoS اور rutas de código empaquetadas دونوں بناتے ہیں؛ Diseño AoS v1 (flags `0x00`) کو configuración predeterminada ہے جب تک banderas de encabezado variantes empaquetadas کو opt-in نہ کریں۔ Implementación `crates/norito_derive/src/derive_struct.rs` میں ہے۔
- Diseño پر اثر انداز ہونے والی características (`packed-struct`, `packed-seq`, `compact-len`) banderas de encabezado کے ذریعے opt-in ہیں اور peers کے درمیان codificación/decodificación consistente ہونی چاہئیں۔
- Ayudantes JSON (`norito::json`) API abiertas کیلئے determinista JSON respaldado por Norito فراہم کرتے ہیں۔ `norito::json::{to_json_pretty, from_json}` استعمال کریں — کبھی `serde_json` نہیں۔

## Tablas de identificadores multicodec

Norito اپنی asignaciones multicódec `norito::multicodec` میں رکھتا ہے۔ Tabla de referencia (hashes, tipos de claves, descriptores de carga útil) raíz del repositorio کے `multicodec.md` میں برقرار رکھی جاتی ہے۔ جب نیا identificador شامل ہو:1. `norito::multicodec::registry` اپڈیٹ کریں۔
2. `multicodec.md` Mesa میں بڑھائیں۔
3. اگر enlaces descendentes (Python/Java) mapa استعمال کرتے ہیں تو انہیں regenerar کریں۔

## Docs اور accesorios کو regenerar کرنا

جب پورٹل اس وقت anfitrión de resumen en prosa کر رہا ہے، fuentes ascendentes de Markdown کو fuente de verdad رکھیں:

- **Especificación**: `norito.md`
- **Tabla multicódec**: `multicodec.md`
- **Puntos de referencia**: `crates/norito/benches/`
- **Pruebas de oro**: `crates/norito/tests/`

جب Docusaurus automatización en vivo ہو جائے، پورٹل ایک script de sincronización کے ذریعے اپڈیٹ ہوگا (جسے `docs/portal/scripts/` میں track کیا گیا ہے) جو ان archivos سے datos کھینچتا ہے۔ تب تک، spec میں تبدیلی کے ساتھ اس صفحہ کو دستی طور پر align رکھیں۔