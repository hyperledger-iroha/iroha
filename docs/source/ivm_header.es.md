---
lang: es
direction: ltr
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T10:30:30.084677+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Encabezado de código de bytes


Magia
- 4 bytes: ASCII `IVM\0` en desplazamiento 0.

Diseño (actual)
- Compensaciones y tamaños (17 bytes en total):
  - 0..4: magia `IVM\0`
  - 4: `version_major: u8`
  - 5: `version_minor: u8`
  - 6: `mode: u8` (bits de característica; ver más abajo)
  - 7: `vector_length: u8`
  - 8..16: `max_cycles: u64` (little-endian)
  - 16: `abi_version: u8`

bits de modo
- `ZK = 0x01`, `VECTOR = 0x02`, `HTM = 0x04` (reservado/con funciones cerradas).

Campos (significado)
- `abi_version`: tabla syscall y versión del esquema puntero-ABI.
- `mode`: bits de característica para calco ZK/VECTOR/HTM.
- `vector_length`: longitud del vector lógico para operaciones vectoriales (0 → sin configurar).
- `max_cycles`: límite de relleno de ejecución utilizado en modo ZK y admisión.

Notas
- La endianidad y el diseño están definidos por la implementación y vinculados a `version`. El diseño en línea anterior refleja la implementación actual en `crates/ivm_abi/src/metadata.rs`.
- Un lector mínimo puede confiar en este diseño para los artefactos actuales y debe manejar cambios futuros mediante la activación `version`.
- La aceleración de hardware (SIMD/Metal/CUDA) es opcional por host. El tiempo de ejecución lee los valores `AccelerationConfig` de `iroha_config`: `enable_simd` fuerza retrocesos escalares cuando es falso, mientras que `enable_metal` e `enable_cuda` controlan sus respectivos backends incluso cuando están compilados. Estos cambios se aplican a través de `ivm::set_acceleration_config` antes de la creación de la VM.
- Los SDK móviles (Android/Swift) muestran los mismos controles; `IrohaSwift.AccelerationSettings`
  llama a `connect_norito_set_acceleration_config` para que las compilaciones de macOS/iOS puedan optar por Metal /
  NEON manteniendo retrocesos deterministas.
- Los operadores también pueden forzar la desactivación de backends específicos para diagnóstico exportando `IVM_DISABLE_METAL=1` o `IVM_DISABLE_CUDA=1`. Estas anulaciones del entorno tienen prioridad sobre la configuración y mantienen la máquina virtual en la ruta determinista de la CPU.

Ayudantes de estado duraderos y superficie ABI
- Las llamadas al sistema auxiliares de estado duradero (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_* y codificación/decodificación JSON/SCHEMA) son parte de la ABI V1 y se incluyen en el cálculo `abi_hash`.
- CoreHost conecta STATE_{GET,SET,DEL} al estado de contrato inteligente duradero respaldado por WSV; Los hosts de desarrollo/prueba pueden usar superposiciones o persistencia local, pero deben preservar el mismo comportamiento observable.

Validación
- La admisión de nodos acepta solo los encabezados `version_major = 1` e `version_minor = 0`.
- `mode` solo debe contener bits conocidos: `ZK`, `VECTOR`, `HTM` (se rechazan los bits desconocidos).
- `vector_length` es de asesoramiento y puede ser distinto de cero incluso si el bit `VECTOR` no está establecido; la admisión impone únicamente un límite superior.
- Valores `abi_version` admitidos: la primera versión acepta solo `1` (V1); otros valores se rechazan en el momento del ingreso.

### Política (generada)
El siguiente resumen de políticas se genera a partir de la implementación y no debe editarse manualmente.<!-- BEGIN GENERATED HEADER POLICY -->
| Campo | Política |
|---|---|
| versión_mayor | 1 |
| versión_menor | 0 |
| modo (bits conocidos) | 0x07 (ZK=0x01, VECTOR=0x02, HTM=0x04) |
| abi_versión | 1 |
| longitud_vectorial | 0 o 1..=64 (aviso; independiente del bit VECTOR) |
<!-- END GENERATED HEADER POLICY -->

### Hashes ABI (generados)
La siguiente tabla se genera a partir de la implementación y enumera los valores canónicos `abi_hash` para las políticas admitidas.

<!-- BEGIN GENERATED ABI HASHES -->
| Política | abi_hash (hexadecimal) |
|---|---|
| ABI v1 | ba1786031c3d0cdbd607debdae1cc611a0807bf9cf49ed349a0632855724969f |
<!-- END GENERATED ABI HASHES -->

- Las actualizaciones menores pueden agregar instrucciones detrás de `feature_bits` y espacio de código de operación reservado; Las actualizaciones importantes pueden cambiar las codificaciones o eliminarlas/reutilizarlas solo junto con una actualización del protocolo.
- Los rangos de llamadas al sistema son estables; desconocido para el activo `abi_version` produce `E_SCALL_UNKNOWN`.
- Los horarios de gas están vinculados al `version` y requieren vectores dorados al cambiar.

Inspeccionar artefactos
- Utilice `ivm_tool inspect <file.to>` para obtener una vista estable de los campos de encabezado.
- Para el desarrollo, los ejemplos incluyen un pequeño destino Makefile `examples-inspect` que ejecuta inspeccionar los artefactos creados.

Ejemplo (Rust): magia mínima + verificación de tamaño

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

Nota: El diseño exacto del encabezado más allá de la magia está versionado y definido por la implementación; prefiera `ivm_tool inspect` para nombres y valores de campos estables.