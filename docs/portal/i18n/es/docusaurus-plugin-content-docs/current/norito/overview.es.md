---
lang: es
direction: ltr
source: docs/portal/docs/norito/overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Resumen de Norito

Norito es la capa de serialización binaria utilizada en todo Iroha: define como se codifican las estructuras de datos en la red, se persisten en disco y se intercambian entre contratos y hosts. Cada caja del espacio de trabajo depende de Norito en lugar de `serde` para que los pares en hardware diferentes produzcan bytes idénticos.

Este resumen sintetiza las piezas centrales y enlaza a las referencias canónicas.

## Arquitectura de un vistazo

- **Cabecera + payload** - Cada mensaje Norito comienza con una cabecera de negociación de características (flags, checksum) seguida del payload sin envolver. Los diseños empaquetados y la compresión se negocian mediante bits de la cabecera.
- **Codificación determinista** - `norito::codec::{Encode, Decode}` implementan la codificación base. El mismo diseño se reutiliza al envolver payloads en cabeceras para que el hash y la firma se mantengan deterministas.
- **Esquema + deriva** - `norito_derive` implementaciones de géneros de `Encode`, `Decode` y `IntoSchema`. Las estructuras/secuencias empaquetadas están habilitadas por defecto y documentadas en `norito.md`.
- **Registro multicodec** - Los identificadores de hashes, tipos de claves y descriptores de carga útil viven en `norito::multicodec`. La tabla autorizada se mantiene en `multicodec.md`.

##Herramientas| Tarea | Comando/API | Notas |
| --- | --- | --- |
| Inspeccionar cabecera/secciones | `ivm_tool inspect <file>.to` | Muestra la versión de ABI, banderas y puntos de entrada. |
| Codificar/decodificar en Rust | `norito::codec::{Encode, Decode}` | Implementado para todos los tipos principales del modelo de datos. |
| JSON de interoperabilidad | `norito::json::{to_json_pretty, from_json}` | JSON determinista respaldado por valores Norito. |
| Generar documentos/especificaciones | `norito.md`, `multicodec.md` | Documentacion fuente de verdad en la raiz del repositorio. |

## Flujo de trabajo de desarrollo

1. **Agregar derivas** - Prefiere `#[derive(Encode, Decode, IntoSchema)]` para nuevas estructuras de datos. Evita serializadores escritos a mano salvo que sea absolutamente necesario.
2. **Validar diseños empaquetados** - Usa `cargo test -p norito` (y la matriz de características empaquetadas en `scripts/run_norito_feature_matrix.sh`) para asegurar que los nuevos diseños se mantengan estables.
3. **Regenerar docs** - Cuando cambie la codificación, actualice `norito.md` y la tabla multicodec, luego refresca las páginas del portal (`/reference/norito-codec` y este resumen).
4. **Mantener pruebas Norito-first** - Las pruebas de integración deben usar los helpers JSON de Norito en lugar de `serde_json` para ejercitar las mismas rutas que producción.

## Enlaces rápidos- Especificación: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Asignaciones multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matriz de características: `scripts/run_norito_feature_matrix.sh`
- Ejemplos de diseño empaquetado: `crates/norito/tests/`

Acompaña este resumen con la guía de inicio rápido (`/norito/getting-started`) para un recorrido práctico de compilar y ejecutar bytecode que usa payloads Norito.