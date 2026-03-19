---
lang: es
direction: ltr
source: docs/source/runtime_upgrades.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee7142a82f21e646d4d71844adbf779d180e5647
source_last_modified: "2025-12-04T06:31:08.260928+00:00"
translation_last_reviewed: 2026-01-01
---

# Actualizaciones de runtime (IVM + Host) - Sin downtime, sin hardfork

Este documento especifica un mecanismo determinista, controlado por gobernanza, para desplegar
actualizaciones de runtime sin detener la red ni hacer hardfork. Los nodos despliegan binarios con
antelacion; la activacion se coordina on-chain dentro de una ventana de altura acotada. Los
contratos antiguos siguen ejecutandose sin cambios; la superficie ABI del host permanece fija en v1.

Nota (primera version): ABI v1 es fijo y no se planean incrementos de version ABI. Los manifests de runtime upgrade deben establecer `abi_version = 1`, y `added_syscalls`/`added_pointer_types` deben estar vacios.

Objetivos
- Activacion determinista en una ventana de altura programada con aplicacion idempotente.
- Preservar la estabilidad de ABI v1; las actualizaciones de runtime no cambian la superficie ABI del host.
- Guardas de admision y ejecucion para que payloads pre-activacion no habiliten nuevo comportamiento.
- Despliegue amigable para operadores con visibilidad de capacidades y modos de falla claros.

No objetivos
- Introducir nuevas versiones ABI o ampliar superficies de syscalls/tipos de puntero (fuera de alcance en esta version).
- Cambiar numeros de syscalls existentes o IDs de tipos de puntero (prohibido).
- Parchear nodos en vivo sin desplegar binarios actualizados.

Definiciones
- Version ABI: entero pequeno declarado en `ProgramMetadata.abi_version` que selecciona un `SyscallPolicy` y una allowlist de tipos de puntero. En la primera version, esto queda fijo en `1`.
- Hash ABI: digest determinista de la superficie ABI para una version dada: lista de syscalls (numeros+formas), IDs/allowlist de tipos de puntero y flags de politica; calculado por `ivm::syscalls::compute_abi_hash`.
- Syscall Policy: mapeo del host que decide si un numero de syscall esta permitido para una version ABI dada y politica del host.
- Activation Window: intervalo semiabierto de altura de bloque `[start, end)` en el que la activacion es valida exactamente una vez en `start`.

Objetos de estado (Modelo de datos)
<!-- BEGIN RUNTIME UPGRADE TYPES -->
- `RuntimeUpgradeId`: Blake2b-256 de los bytes Norito canonicos de un manifest.
- Campos de `RuntimeUpgradeManifest`:
  - `name: String` - etiqueta legible.
  - `description: String` - descripcion corta para operadores.
  - `abi_version: u16` - version ABI objetivo a activar (debe ser 1 en la primera version).
  - `abi_hash: [u8; 32]` - hash ABI canonico para la politica objetivo.
  - `added_syscalls: Vec<u16>` - lista delta reservada; debe quedar vacia en la primera version.
  - `added_pointer_types: Vec<u16>` - lista delta reservada; debe quedar vacia en la primera version.
  - `start_height: u64` - primera altura de bloque donde se permite la activacion.
  - `end_height: u64` - limite superior exclusivo de la ventana de activacion.
  - `sbom_digests: Vec<RuntimeUpgradeSbomDigest>` - digests SBOM para artefactos de actualizacion.
  - `slsa_attestation: Vec<u8>` - bytes crudos de atestacion SLSA (base64 en JSON).
  - `provenance: Vec<ManifestProvenance>` - firmas sobre el payload canonico.
- Campos de `RuntimeUpgradeRecord`:
  - `manifest: RuntimeUpgradeManifest` - payload canonico de propuesta.
  - `status: RuntimeUpgradeStatus` - estado del ciclo de vida de la propuesta.
  - `proposer: AccountId` - autoridad que envio la propuesta.
  - `created_height: u64` - altura donde la propuesta entro en el ledger.
- Campos de `RuntimeUpgradeSbomDigest`:
  - `algorithm: String` - identificador del algoritmo de digest.
  - `digest: Vec<u8>` - bytes crudos del digest (base64 en JSON).
<!-- END RUNTIME UPGRADE TYPES -->
  - Invariantes: `end_height > start_height`; `abi_version` debe ser `1`; `abi_hash` debe ser igual a `ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)`; `added_*` debe estar vacio; los numeros/IDs existentes NO deben eliminarse ni renumerarse.

Diseno de almacenamiento
- `world.runtime_upgrades`: mapa MVCC con clave `RuntimeUpgradeId.0` (hash crudo de 32 bytes) y valores codificados como payloads Norito canonicos `RuntimeUpgradeRecord`. Las entradas persisten entre bloques; los commits son idempotentes y seguros ante replays.

Instrucciones (ISI)
- ProposeRuntimeUpgrade { manifest: RuntimeUpgradeManifest }
  - Efectos: inserta `RuntimeUpgradeRecord { status: Proposed }` con clave `RuntimeUpgradeId` si no existe.
  - Rechaza si la ventana se solapa con otro registro Proposed/Activated o si fallan las invariantes.
  - Idempotente: re-enviar los mismos bytes canonicos del manifest no hace nada.
  - Codificacion canonica: los bytes del manifest deben coincidir con `RuntimeUpgradeManifest::canonical_bytes()`; codificaciones no canonicas se rechazan.
- ActivateRuntimeUpgrade { id: RuntimeUpgradeId }
  - Precondiciones: existe un registro Proposed coincidente; `current_height` debe ser igual a `manifest.start_height`; `current_height < manifest.end_height`.
  - Efectos: cambia el registro a `ActivatedAt(current_height)`; el conjunto ABI activo permanece `{1}` en la primera version.
  - Idempotente: replays en la misma altura son no-ops; otras alturas se rechazan de forma determinista.
- CancelRuntimeUpgrade { id: RuntimeUpgradeId }
  - Precondiciones: el estado es Proposed y `current_height < manifest.start_height`.
  - Efectos: cambia a `Canceled`.

Eventos (Data Events)
- RuntimeUpgradeEvent::{Proposed { id, manifest }, Activated { id, abi_version, at_height }, Canceled { id }}

Reglas de admision
- Admision de contratos: en la primera version solo se acepta `ProgramMetadata.abi_version = 1`; otros valores se rechazan con `IvmAdmissionError::UnsupportedAbiVersion`.
  - Para ABI v1, recomputar `abi_hash(1)` y exigir igualdad con payload/manifest cuando se provea; si no, rechazar con `IvmAdmissionError::ManifestAbiHashMismatch`.
- Admision de transacciones: las instrucciones `ProposeRuntimeUpgrade`/`ActivateRuntimeUpgrade`/`CancelRuntimeUpgrade` requieren permisos apropiados (root/sudo); deben cumplir las restricciones de solape de ventanas.

Aplicacion de provenance
- Los manifests de runtime upgrade pueden incluir digests SBOM (`sbom_digests`), bytes de atestacion SLSA (`slsa_attestation`), y metadatos de firmantes (firmas `provenance`). Las firmas cubren el `RuntimeUpgradeManifestSignaturePayload` canonico (todos los campos del manifest excepto la lista de firmas `provenance`).
- La configuracion de governance controla la aplicacion bajo `governance.runtime_upgrade_provenance`:
  - `mode`: `optional` (acepta ausencia de provenance, verifica si esta presente) o `required` (rechaza si falta provenance).
  - `require_sbom`: cuando `true`, se requiere al menos un digest SBOM.
  - `require_slsa`: cuando `true`, se requiere una atestacion SLSA no vacia.
  - `trusted_signers`: lista de claves publicas de firmantes aprobados.
  - `signature_threshold`: numero minimo de firmas confiables requeridas.
- Los rechazos de provenance exponen codigos de error estables en fallos de instrucciones (prefijo `runtime_upgrade_provenance:`):
  - `missing_provenance`, `missing_sbom`, `invalid_sbom_digest`, `missing_slsa_attestation`
  - `missing_signatures`, `invalid_signature`, `untrusted_signer`, `signature_threshold_not_met`
- Telemetria: `runtime_upgrade_provenance_rejections_total{reason}` cuenta razones de rechazo de provenance.

Reglas de ejecucion
- Politica del host VM: durante la ejecucion del programa, derive `SyscallPolicy` desde `ProgramMetadata.abi_version`. Syscalls desconocidos para esa version mapean a `VMError::UnknownSyscall`.
- Pointer-ABI: allowlist derivada de `ProgramMetadata.abi_version`; los tipos fuera de la allowlist para esa version se rechazan durante decode/validacion.
- Cambio de host: cada bloque recomputa el conjunto ABI activo; en la primera version permanece `{1}`, pero la activacion se registra y es idempotente (validado por `runtime_upgrade_admission::activation_allows_v1_in_same_block`).
  - Binding de politica de syscalls: `CoreHost` lee la version ABI declarada por la transaccion y aplica `ivm::syscalls::is_syscall_allowed`/`is_type_allowed_for_policy` contra el `SyscallPolicy` por bloque. El host reutiliza la instancia VM con alcance de transaccion, por lo que activaciones a mitad de bloque son seguras: transacciones posteriores observan la politica actualizada mientras las anteriores continuan con su version original.

Invariantes de determinismo y seguridad
- La activacion ocurre solo en `start_height` y es idempotente; los reorgs por debajo de `start_height` reaplican deterministicamente una vez que el bloque vuelve a aterrizar.
- El conjunto ABI activo esta fijo en `{1}` en la primera version.
- Ninguna negociacion dinamica influye en el consenso o en el orden de ejecucion; el gossip de capacidades es solo informativo.

Despliegue del operador (sin downtime)
1) Desplegar un binario de nodo que incluya el nuevo artefacto de runtime y mantenga ABI v1.
2) Observar la preparacion de la flota via telemetria.
3) Enviar `ProposeRuntimeUpgrade` con una ventana suficientemente adelantada (p. ej., `H+N`).
4) En `start_height`, `ActivateRuntimeUpgrade` se ejecuta como parte del bloque incluido y registra la activacion; ABI sigue en v1.

Torii y CLI
- Torii
  - `GET /v1/runtime/abi/active` -> `{ abi_version: u16 }` (implementado)
  - `GET /v1/runtime/abi/hash` -> `{ policy: "V1", abi_hash_hex: "<64-hex>" }` (implementado)
  - `GET /v1/runtime/upgrades` -> lista de registros (implementado).
  - `POST /v1/runtime/upgrades/propose` -> envuelve `ProposeRuntimeUpgrade` (devuelve esqueleto de instruccion; implementado).
  - `POST /v1/runtime/upgrades/activate/:id` -> envuelve `ActivateRuntimeUpgrade` (devuelve esqueleto de instruccion; implementado).
  - `POST /v1/runtime/upgrades/cancel/:id` -> envuelve `CancelRuntimeUpgrade` (devuelve esqueleto de instruccion; implementado).
- CLI
  - `iroha runtime abi active` (implementado)
  - `iroha runtime abi hash` (implementado)
  - `iroha runtime upgrade list` (implementado)
  - `iroha runtime upgrade propose --file <manifest.json>` (implementado)
  - `iroha runtime upgrade activate --id <id>` (implementado)
  - `iroha runtime upgrade cancel --id <id>` (implementado)

API de consultas core
- Consulta Norito singular (firmada):
  - `FindAbiVersion` devuelve una estructura Norito `{ abi_version: u16 }`.
  - Ver ejemplo: `docs/source/samples/find_active_abi_versions.md` (tipo/campos y ejemplo JSON).

Notas de implementacion (solo v1)
- iroha_data_model
  - Agregar `RuntimeUpgradeManifest`, `RuntimeUpgradeRecord`, enums de instrucciones, eventos y codecs JSON/Norito con pruebas de roundtrip.
- iroha_core
  - WSV: agregar registro `runtime_upgrades` con checks de solape y getters.
  - Executors: implementar handlers de ISI; emitir eventos; aplicar reglas de admision.
  - Admission: gatear manifests de programa por actividad de `abi_version` y igualdad de `abi_hash`.
  - Mapeo de politica de syscalls: pasar el conjunto ABI activo al constructor del host VM; garantizar determinismo usando la altura del bloque al inicio de la ejecucion.
  - Tests: idempotencia de ventana de activacion, rechazos por solape, comportamiento de admision pre/post.
- ivm
  - La superficie ABI esta fija en v1; las listas de syscalls y hashes ABI estan fijados por pruebas golden.
- iroha_cli / iroha_torii
  - Agregar endpoints y comandos listados arriba; helpers Norito JSON para manifests; pruebas de integracion basicas.
- Kotodama compiler
  - Emite `abi_version = 1` e incrusta el `abi_hash` canonico v1 en manifests `.to`.

Telemetria
- Agregar gauge `runtime.abi_version` y counter `runtime.upgrade_events_total{kind}`.

Consideraciones de seguridad
- Solo root/sudo puede proponer/activar/cancelar; los manifests deben estar firmados adecuadamente.
- Las ventanas de activacion evitan front-running y aseguran aplicacion determinista.
- `abi_hash` fija la superficie de interfaz para evitar drift silencioso entre binarios.

Criterios de aceptacion (Conformance)
- Los nodos rechazan deterministicamente codigo con `abi_version != 1` en todo momento.
- Las actualizaciones de runtime no cambian la politica ABI; los programas existentes siguen funcionando sin cambios con v1.
- Las pruebas golden para hashes ABI y listas de syscalls pasan en x86-64/ARM64.
- La activacion es idempotente y segura bajo reorgs.
