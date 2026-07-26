---
lang: es
direction: ltr
source: docs/portal/docs/norito/overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Objeto Norito

Norito — serie binaria de estilo binario, incluida en el conjunto Iroha: según la estructura original кодируются в сети, сохраняются на diske и обмениваются между контрактами и хостами. Каждый crate в workspace опирается на Norito вместо `serde`, чтобы пиры на разном оборудовании производили идентичные байты.

Esto se debe a que las teclas y los materiales canónicos están conectados.

## Arquitecto en obras de arte

- **Заголовок + payload** – El archivo Norito muestra las características de configuración del archivo (banderas, suma de comprobación), para el resto de la carga útil. Упакованные раскладки и сжатие согласуются через биты заголовка.
- **Детерминированное кодирование** – `norito::codec::{Encode, Decode}` realiza una rápida codificación. Este diseño se aplica a la configuración de cargas útiles en los contenedores, la configuración y la configuración de determinados parámetros.
- **Схема + deriva** – `norito_derive` генерирует реализации `Encode`, `Decode` и `IntoSchema`. Las estructuras/posteriores actualizadas según las versiones y descripciones de `norito.md`.
- **Реестр multicodec** – Identificadores de datos, tipos de claves y carga útil opcional incluidos en `norito::multicodec`. La tabla de autorización se encuentra en `multicodec.md`.

## Instrumentos| Задача | Comando / API | Примечания |
| --- | --- | --- |
| Проверить заголовок/секции | `ivm_tool inspect <file>.to` | Показывает версию ABI, banderas y puntos de entrada. |
| Codificar/decorar en Rust | `norito::codec::{Encode, Decode}` | Realización de todos los modelos de datos existentes. |
| JSON de interoperabilidad | `norito::json::{to_json_pretty, from_json}` | El archivo JSON predeterminado está en el archivo Norito. |
| Generar documentos/especificaciones | `norito.md`, `multicodec.md` | La documentación histórica está en el repositorio principal. |

## Процесс разработки

1. **Добавить deriva** – Предпочитайте `#[derive(Encode, Decode, IntoSchema)]` для новых структур данных. Hay muchos serializadores que no son absolutamente necesarios.
2. **Mostrar diseños mejorados** – Utilice `cargo test -p norito` (y funciones empaquetadas en `scripts/run_norito_feature_matrix.sh`), qué diseños nuevos tiene остаются стабильными.
3. **Consultar documentos**: seleccione la lista de códigos, abra `norito.md` y la tabla multicodec, consulte las páginas del portal. (`/reference/norito-codec` y esta observación).
4. **Держать тесты Norito-first** – Las pruebas integradas utilizan ayudas JSON Norito en todo `serde_json`, чтобы проходить те же пути, что и продакшн.

## Быстрые ссылки

- Especificaciones: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Nombre multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Características del script матрицы: `scripts/run_norito_feature_matrix.sh`
- Primeros diseños empaquetados: `crates/norito/tests/`Conecte esta opción con el inicio del sistema operativo (`/norito/getting-started`) para una compilación práctica de programas y aplicaciones байткода, использующего cargas útiles Norito.