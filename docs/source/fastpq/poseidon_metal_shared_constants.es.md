---
lang: es
direction: ltr
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2026-01-03T18:07:57.621942+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Constantes compartidas de Poseidon Metal

Los núcleos metálicos, los núcleos CUDA, el probador Rust y cada dispositivo SDK deben compartir
exactamente los mismos parámetros de Poseidon2 para mantener la aceleración por hardware
hash determinista. Este documento registra la instantánea canónica, cómo
regenerarlo y cómo se espera que las canalizaciones de GPU ingieran los datos.

## Manifiesto de instantánea

Los parámetros se publican como documento `PoseidonSnapshot` RON. Las copias son
mantenido bajo control de versiones para que las cadenas de herramientas de GPU y los SDK no dependan del tiempo de compilación
generación de código.

| Camino | Propósito | SHA-256 |
|------|---------|---------|
| `artifacts/offline_poseidon/constants.ron` | Instantánea canónica generada a partir de `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}`; fuente de verdad para las compilaciones de GPU. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` | Refleja la instantánea canónica para que las pruebas unitarias de Swift y el arnés de humo XCFramework carguen las mismas constantes que esperan los núcleos de Metal. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | Los dispositivos Android/Kotlin comparten el mismo manifiesto para las pruebas de paridad y serialización. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

Cada consumidor debe verificar el hash antes de conectar las constantes a una GPU.
tubería. Cuando el manifiesto cambia (nuevo conjunto de parámetros o perfil), el SHA y
los espejos posteriores deben actualizarse al mismo tiempo.

## Regeneración

El manifiesto se genera a partir de las fuentes de Rust ejecutando `xtask`.
ayudante. El comando escribe tanto el archivo canónico como los espejos del SDK:

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

Utilice `--constants <path>`/`--vectors <path>` para anular los destinos o
`--no-sdk-mirror` al regenerar solo la instantánea canónica. El ayudante
reflejar los artefactos en los árboles Swift y Android cuando se omite la bandera,
lo que mantiene los hashes alineados para CI.

## Alimentación de construcciones de metal/CUDA

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` y
  `crates/fastpq_prover/cuda/fastpq_cuda.cu` debe regenerarse desde el
  se manifiesta cada vez que cambia la tabla.
- Las constantes redondeadas y MDS se organizan en `MTLBuffer`/`__constant` contiguas
  segmentos que coinciden con el diseño del manifiesto: `round_constants[round][state_width]`
  seguido de la matriz MDS 3x3.
- `fastpq_prover::poseidon_manifest()` carga y valida la instantánea en
  tiempo de ejecución (durante el calentamiento del metal) para que las herramientas de diagnóstico puedan afirmar que el
  las constantes del sombreador coinciden con el hash publicado a través de
  `fastpq_prover::poseidon_manifest_sha256()`.
- Lectores de dispositivos SDK (Swift `PoseidonSnapshot`, Android `PoseidonSnapshot`) y
  las herramientas fuera de línea Norito se basan en el mismo manifiesto, lo que evita que solo la GPU
  horquillas de parámetros.

## Validación

1. Después de regenerar el manifiesto, ejecute `cargo test -p xtask` para ejercitar el
   Pruebas unitarias de generación de dispositivos Poseidon.
2. Registre el nuevo SHA-256 en este documento y en cualquier panel que monitoree
   Artefactos de GPU.
3. Análisis `cargo test -p fastpq_prover poseidon_manifest_consistency`
   `poseidon2.metal` e `fastpq_cuda.cu` en el momento de la compilación y afirma que sus
   Las constantes serializadas coinciden con el manifiesto, manteniendo las tablas CUDA/Metal y
   la instantánea canónica al mismo tiempo.Mantener el manifiesto junto con las instrucciones de compilación de la GPU le da a Metal/CUDA
flujos de trabajo un apretón de manos determinista: los núcleos son libres de optimizar su memoria
diseño siempre que ingieran el blob de constantes compartidas y expongan el hash en
Telemetría para controles de paridad.