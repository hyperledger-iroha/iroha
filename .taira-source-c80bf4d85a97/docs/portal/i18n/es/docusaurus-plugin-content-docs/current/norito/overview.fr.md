---
lang: es
direction: ltr
source: docs/portal/docs/norito/overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Vista del conjunto de Norito

Norito es la base de serialización binaria utilizada en todo Iroha: definitivamente comenta las estructuras de las personas que están codificadas en el archivo, persisten en el disco y se intercambian entre contratos y hotes. Cada caja del espacio de trabajo se aplica en Norito plutot que en `serde` afin de pares en el material diferentes que producen octetos idénticos.

Cet apercu resume les elements cles et renvoie aux references canonices.

## Arquitectura en un golpe de ojo

- **En-tete + payload** - Cada mensaje Norito comienza por un en-tete de negociación de características (flags, checksum) después de la carga útil brut. Los paquetes de diseño y la compresión se negocian a través de los bits del dispositivo.
- **Determinista de codificación** - `norito::codec::{Encode, Decode}` implementa la codificación nula. El diseño del meme se reutiliza cuando se guardan las cargas útiles en las manos para que el hachage y la firma queden determinados.
- **Esquema + derivaciones** - `norito_derive` genera las implementaciones `Encode`, `Decode` e `IntoSchema`. Los paquetes de estructuras/secuencias están activos por defecto y documentados en `norito.md`.
- **Registro multicodec** - Los identificadores de hashes, tipos de clave y descriptores de carga útil se encuentran en `norito::multicodec`. La tabla de referencia se mantiene en `multicodec.md`.

## Herramientas| Taché | Comando / API | Notas |
| --- | --- | --- |
| Inspector l'en-tete/secciones | `ivm_tool inspect <file>.to` | Muestra la versión ABI, las banderas y los puntos de entrada. |
| Codificador/decodificador en Rust | `norito::codec::{Encode, Decode}` | Implemente todos los tipos de principios del modelo de datos. |
| JSON de interoperabilidad | `norito::json::{to_json_pretty, from_json}` | JSON determina dos valores de valor Norito. |
| Documentos/especificaciones generales | `norito.md`, `multicodec.md` | Documentación fuente de verité a la racine du repo. |

## Flujo de trabajo de desarrollo

1. **Agregar derivados** - Preferez `#[derive(Encode, Decode, IntoSchema)]` pour les nouvellesstructures de donnees. Evitez les serialiseurs ecrits a la main sauf necessite absolue.
2. **Valider los paquetes de diseños** - Utilice `cargo test -p norito` (y la matriz de paquetes de características en `scripts/run_norito_feature_matrix.sh`) para garantizar que los nuevos diseños permanezcan estables.
3. **Regenerar los documentos** - Cuando el cambio de codificación, agregue un día `norito.md` y la tabla multicodec, luego rafraichissez las páginas del portal (`/reference/norito-codec` y ese acceso).
4. **Guarde las pruebas Norito-first** - Las pruebas de integración deben utilizar los ayudantes JSON Norito en lugar de `serde_json` para ejercitar los memes chemins que la producción.

## Gravámenes rápidos- Especificación: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Atribuciones multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matriz de características: `scripts/run_norito_feature_matrix.sh`
- Ejemplos de paquete de diseño: `crates/norito/tests/`

Asocié esta apertura a la guía de inicio rápido (`/norito/getting-started`) para un recorrido práctico de compilación y ejecución de código de bytes utilizando las cargas útiles Norito.