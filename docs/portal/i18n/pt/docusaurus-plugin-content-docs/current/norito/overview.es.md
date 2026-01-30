---
lang: pt
direction: ltr
source: docs/portal/docs/norito/overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Resumen de Norito

Norito es la capa de serializacion binaria utilizada en todo Iroha: define como se codifican las estructuras de datos en la red, se persisten en disco y se intercambian entre contratos y hosts. Cada crate del workspace depende de Norito en lugar de `serde` para que los peers en hardware diferente produzcan bytes identicos.

Este resumen sintetiza las piezas centrales y enlaza a las referencias canonicas.

## Arquitectura de un vistazo

- **Cabecera + payload** - Cada mensaje Norito comienza con una cabecera de negociacion de features (flags, checksum) seguida del payload sin envolver. Los layouts empaquetados y la compresion se negocian mediante bits de la cabecera.
- **Codificacion determinista** - `norito::codec::{Encode, Decode}` implementan la codificacion base. El mismo layout se reutiliza al envolver payloads en cabeceras para que el hashing y la firma se mantengan deterministas.
- **Esquema + derives** - `norito_derive` genera implementaciones de `Encode`, `Decode` y `IntoSchema`. Los structs/secuencias empaquetados estan habilitados por defecto y documentados en `norito.md`.
- **Registro multicodec** - Los identificadores de hashes, tipos de clave y descriptores de payload viven en `norito::multicodec`. La tabla autorizada se mantiene en `multicodec.md`.

## Herramientas

| Tarea | Comando / API | Notas |
| --- | --- | --- |
| Inspeccionar cabecera/secciones | `ivm_tool inspect <file>.to` | Muestra la version de ABI, flags y entrypoints. |
| Codificar/decodificar en Rust | `norito::codec::{Encode, Decode}` | Implementado para todos los tipos principales del data model. |
| Interop JSON | `norito::json::{to_json_pretty, from_json}` | JSON determinista respaldado por valores Norito. |
| Generar docs/especificaciones | `norito.md`, `multicodec.md` | Documentacion fuente de verdad en la raiz del repo. |

## Flujo de trabajo de desarrollo

1. **Agregar derives** - Prefiere `#[derive(Encode, Decode, IntoSchema)]` para nuevas estructuras de datos. Evita serializadores escritos a mano salvo que sea absolutamente necesario.
2. **Validar layouts empaquetados** - Usa `cargo test -p norito` (y la matriz de features empaquetadas en `scripts/run_norito_feature_matrix.sh`) para asegurar que los nuevos layouts se mantengan estables.
3. **Regenerar docs** - Cuando cambie la codificacion, actualiza `norito.md` y la tabla multicodec, luego refresca las paginas del portal (`/reference/norito-codec` y este resumen).
4. **Mantener pruebas Norito-first** - Las pruebas de integracion deben usar los helpers JSON de Norito en lugar de `serde_json` para ejercitar las mismas rutas que produccion.

## Enlaces rapidos

- Especificacion: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Asignaciones multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matriz de features: `scripts/run_norito_feature_matrix.sh`
- Ejemplos de layout empaquetado: `crates/norito/tests/`

Acompana este resumen con la guia de inicio rapido (`/norito/getting-started`) para un recorrido practico de compilar y ejecutar bytecode que usa payloads Norito.
