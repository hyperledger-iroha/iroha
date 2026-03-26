---
lang: es
direction: ltr
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:01:14.866000+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Llamada al sistema ABI

Este documento define los números de llamada al sistema IVM, las convenciones de llamada de puntero-ABI, los rangos de números reservados y la tabla canónica de llamadas al sistema orientadas al contrato utilizadas por la reducción de Kotodama. Complementa `ivm.md` (arquitectura) e `kotodama_grammar.md` (idioma).

Versionado
- El conjunto de llamadas al sistema reconocidas depende del campo `abi_version` del encabezado del código de bytes. La primera versión acepta sólo `abi_version = 1`; otros valores se rechazan en el momento del ingreso. Números desconocidos para el `abi_version` activo atrapan de manera determinista con `E_SCALL_UNKNOWN`.
- Las actualizaciones en tiempo de ejecución mantienen `abi_version = 1` y no expanden las superficies syscall o pointer-ABI.
- Los costos de gas de Syscall son parte del programa de gas versionado vinculado a la versión del encabezado del código de bytes. Ver `ivm.md` (Política de gas).

Rangos de numeración
- `0x00..=0x1F`: núcleo/utilidad de VM (los asistentes de depuración/salida están disponibles en `CoreHost`; los asistentes de desarrollo restantes son solo de host simulado).
- `0x20..=0x5F`: Puente ISI central Iroha (estable en ABI v1).
- `0x60..=0x7F`: la extensión ISI está controlada por las características del protocolo (aún forma parte de ABI v1 cuando está habilitada).
- `0x80..=0xFF`: host/ayudantes criptográficos y ranuras reservadas; solo se aceptan los números presentes en la lista de permitidos de ABI v1.

Ayudantes duraderos (ABI v1)
- Las llamadas al sistema auxiliares de estado duradero (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, codificación/decodificación JSON/SCHEMA) son parte de la ABI V1 y se incluyen en el cálculo `abi_hash`.
- CoreHost conecta STATE_{GET,SET,DEL} al estado de contrato inteligente duradero respaldado por WSV; Los hosts de desarrollo/prueba pueden persistir localmente pero deben conservar una semántica de llamada al sistema idéntica.

Convención de llamadas de puntero-ABI (llamadas al sistema de contrato inteligente)
- Los argumentos se colocan en los registros `r10+` como valores `u64` sin procesar o como punteros a la región INPUT para envolventes TLV Norito inmutables (por ejemplo, `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`).
- Los valores de retorno escalares son los `u64` devueltos por el host. El host escribe los resultados del puntero en `r10`.

Tabla de llamadas al sistema canónicas (subconjunto)| Hexágono | Nombre | Argumentos (en `r10+`) | Devoluciones | Gas (base + variable) | Notas |
|------|----------------------|-------------------------------------------------------------------------|-------------|------------------------------|-------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`, `&Name`, `&Json` | `u64=0` | `G_set_detail + bytes(val)` | Escribe un detalle para la cuenta |
| 0x22 | MINT_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | Casas de moneda `amount` de activo a cuenta |
| 0x23 | QUEMAR_ACTIVO | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | Quema `amount` de la cuenta |
| 0x24 | TRANSFERIR_ACTIVO | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | Transferencias `amount` entre cuentas |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | – | `u64=0` | `G_transfer` | Comenzar el alcance del lote de transferencia FASTPQ |
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | Vaciar lote de transferencia FASTPQ acumulado |
| 0x2B | TRANSFER_V1_BATCH_APPLY | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | Aplique un lote codificado con Norito en una única llamada al sistema |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | Registra un nuevo NFT |
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`, `&NftId`, `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | Transfiere la propiedad de NFT |
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | Actualiza los metadatos de NFT |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | Quema (destruye) un NFT |
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | Las consultas iterables se ejecutan de forma efímera; `QueryRequest::Continue` rechazado |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS | – | `u64=count` | `G_create_nfts_for_all` | Ayudante; con funciones cerradas || 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | Administración; con funciones cerradas |
| 0xA4 | GET_AUTHORITY | – (el anfitrión escribe el resultado) | `&AccountId`| `G_get_auth` | El host escribe un puntero a la autoridad actual en `r10` |
| 0xF7 | GET_MERKLE_PATH | `addr:u64`, `out_ptr:u64`, opcional `root_out:u64` | `u64=len` | `G_mpath + len` | Escribe la ruta (hoja → raíz) y bytes raíz opcionales |
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`, `out_ptr:u64`, opcional `depth_cap:u64`, opcional `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`, `out_ptr:u64`, opcional `depth_cap:u64`, opcional `root_out:u64` | `u64=depth` | `G_mpath + depth` | Mismo diseño compacto para el compromiso de registro |

Control de gases
- CoreHost cobra gas adicional por las llamadas al sistema ISI utilizando el programa ISI nativo; Las transferencias por lotes FASTPQ se cobran por entrada.
- Las llamadas al sistema ZK_VERIFY reutilizan el programa de verificación de gas confidencial (base + tamaño de prueba).
- SMARTCONTRACT_EXECUTE_QUERY cobra base + por artículo + por byte; la clasificación multiplica el costo por artículo y las compensaciones sin clasificar agregan una penalización por artículo.

Notas
- Todos los argumentos de puntero hacen referencia a sobres TLV Norito en la región INPUT y se validan en la primera desreferencia (`E_NORITO_INVALID` en caso de error).
- Todas las mutaciones se aplican a través del ejecutor estándar de Iroha (a través de `CoreHost`), no directamente por la VM.
- Las constantes exactas del gas (`G_*`) están definidas por el programa de gas activo; ver `ivm.md`.

Errores
- `E_SCALL_UNKNOWN`: número de llamada al sistema no reconocido para el `abi_version` activo.
- Los errores de validación de entrada se propagan como capturas de VM (por ejemplo, `E_NORITO_INVALID` para TLV con formato incorrecto).

Referencias cruzadas
- Arquitectura y semántica de VM: `ivm.md`
- Idioma y mapeo integrado: `docs/source/kotodama_grammar.md`

nota generacional
- Se puede generar una lista completa de constantes de syscall desde la fuente con:
  - `make docs-syscalls` → escribe `docs/source/ivm_syscalls_generated.md`
  - `make check-docs` → verifica que la tabla generada esté actualizada (útil en CI)
- El subconjunto anterior sigue siendo una tabla estable y seleccionada para llamadas al sistema orientadas a contratos.

## Ejemplos de TLV de administrador/rol (host simulado)

Esta sección documenta las formas de TLV y las cargas útiles JSON mínimas aceptadas por el host WSV simulado para las llamadas al sistema de estilo administrador utilizadas en las pruebas. Todos los argumentos del puntero siguen el puntero ABI (sobres TLV Norito colocados en INPUT). Los hosts de producción pueden utilizar esquemas más completos; Estos ejemplos tienen como objetivo aclarar los tipos y las formas básicas.- REGISTER_PEER / UNREGISTER_PEER
  - Argumentos: `r10=&Json`
  - Ejemplo JSON: `{ "peer": "peer-id-or-info" }`
  - Nota de CoreHost: `REGISTER_PEER` espera un objeto JSON `RegisterPeerWithPop` con bytes `peer` + `pop` (opcional `activation_at`, `expiry_at`, `hsm`); `UNREGISTER_PEER` acepta una cadena de identificación de igual o `{ "peer": "..." }`.

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  -CREAR_TRIGGER:
    - Argumentos: `r10=&Json`
    - JSON mínimo: `{ "name": "t1" }` (campos adicionales ignorados por el simulacro)
  - REMOVE_TRIGGER:
    - Argumentos: `r10=&Name` (nombre del activador)
  - SET_TRIGGER_ENABLED:
    - Argumentos: `r10=&Name`, `r11=enabled:u64` (0 = deshabilitado, distinto de cero = habilitado)
  - Nota de CoreHost: `CREATE_TRIGGER` espera una especificación de activación completa (cadena base64 Norito `Trigger` o
    `{ "id": "<trigger_id>", "action": ... }` con `action` como cadena base64 Norito `Action` o
    un objeto JSON), y `SET_TRIGGER_ENABLED` alterna la clave de metadatos del activador `__enabled` (falta
    por defecto está habilitado).

- Roles: CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  -CREAR_ROLE:
    - Args: `r10=&Name` (nombre de rol), `r11=&Json` (conjunto de permisos)
    - JSON acepta la clave `"perms"` o `"permissions"`, cada una de las cuales es una matriz de cadenas de nombres de permisos.
    - Ejemplos:
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:<i105-account-id>", "transfer_asset:rose#wonder" ] }`
    - Prefijos de nombres de permisos admitidos en el simulacro:
      - `register_domain`, `register_account`, `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - ELIMINAR_ROLE:
    - Argumentos: `r10=&Name`
    - Falla si alguna cuenta todavía tiene asignada esta función.
  - GRANT_ROLE / REVOKE_ROLE:
    - Args: `r10=&AccountId` (asunto), `r11=&Name` (nombre del rol)
  - Nota de CoreHost: el permiso JSON puede ser un objeto `Permission` completo (`{ "name": "...", "payload": ... }`) o una cadena (la carga útil por defecto es `null`); `GRANT_PERMISSION`/`REVOKE_PERMISSION` acepta `&Name` o `&Json(Permission)`.

- Cancelar operaciones (dominio/cuenta/activo): invariantes (simulacros)
  - UNREGISTER_DOMAIN (`r10=&DomainId`) falla si existen cuentas o definiciones de activos en el dominio.
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) falla si la cuenta tiene saldos distintos de cero o posee NFT.
  - UNREGISTER_ASSET (`r10=&AssetDefinitionId`) falla si existe algún saldo para el activo.

Notas
- Estos ejemplos reflejan el host WSV simulado utilizado en las pruebas; Los hosts de nodos reales pueden exponer esquemas de administración más completos o requerir validación adicional. Las reglas de puntero-ABI aún se aplican: los TLV deben estar en INPUT, versión = 1, los ID de tipo deben coincidir y los hash de carga útil deben validarse.