---
lang: es
direction: ltr
source: docs/portal/docs/reference/norito-codec.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Справочник кодека Norito

Norito — канонический слой сериализации Iroha. La conexión en línea, la carga útil en el disco y la API integrada utilizan Norito, que son aplicaciones de conexión en bloques idénticos. даже при разном оборудовании. Esta página puede seleccionar elementos clave y utilizar las especificaciones técnicas de `norito.md`.

## Базовая компоновка

| Componente | Назначение | Источник |
| --- | --- | --- |
| **Encabezado** | Обрамляет payloads magic/version/schema hash, CRC64, длиной и тегом сжатия; v1 incluye `VERSION_MINOR = 0x00` y protege los indicadores de encabezado de las máscaras de protección originales (con el nombre de `0x00`). | `norito::header` — см. `norito.md` ("Encabezado y banderas", корень репозитория) |
| **Carga útil desnuda** | Детерминированное кодирование значений для hashing/сравнения. Encabezado de transporte en línea; bare байты — только внутренние. | `norito::codec::{Encode, Decode}` |
| **Compresión** | El Zstd opcional (y el procesador de GPU experimental), se coloca en el encabezado. | `norito.md`, “Negociación de compresión” |

Los indicadores de diseño principales (estructura empaquetada, secuencia empaquetada, conjunto de bits de campo, longitudes compactas) están disponibles en `norito::header::flags`. V1 utiliza flags `0x00`, no hay flags en las máscaras de poder anteriores; неизвестные биты отклоняются. `norito::header::Flags` сохраняется для внутренней инспекции и будущих версий.

## Поддержка derivar`norito_derive` proporciona derivaciones `Encode`, `Decode`, `IntoSchema` y ayudantes JSON. Ключевые соглашения:

- Derivar rutas de código genéricas como AoS y empaquetadas; La versión 1 utiliza el diseño AoS (flags `0x00`), y los flags de encabezado no incluyen variantes empaquetadas. Реализация находится в `crates/norito_derive/src/derive_struct.rs`.
- Funciones, configuración de diseño (`packed-struct`, `packed-seq`, `compact-len`), inclusión voluntaria de banderas de encabezado y opciones кодироваться/декодироваться согласованно между pares.
- Ayudantes JSON (`norito::json`) que son compatibles con JSON respaldado por Norito para una API pública. Utilice `norito::json::{to_json_pretty, from_json}` — никогда `serde_json`.

## Multicodec y tablas de identificación

Norito está conectado al multicódec en `norito::multicodec`. Las tablas de referencia (hashes, tipos de claves, carga útil de los descriptores) se pueden guardar en `multicodec.md` en el repositorio principal. При добавлении нового идентификатора:

1. Actualice `norito::multicodec::registry`.
2. Limpie la tabla en `multicodec.md`.
3. Utilice enlaces descendentes (Python/Java) o utilice este mapa.

## Перегенерация documentos y accesorios

Para obtener más información sobre las opciones de este portal, utilice las listas de Markdown ascendentes para las siguientes listas:

- **Especificación**: `norito.md`
- **Tabla multicódec**: `multicodec.md`
- **Puntos de referencia**: `crates/norito/benches/`
- **Pruebas de oro**: `crates/norito/tests/`Para la automatización Docusaurus, se incluye un script de sincronización portátil (incluido en `docs/portal/scripts/`), código извлекает данные из этих файлов. Asegúrese de que esta página esté sincronizada con cada una de las características específicas.