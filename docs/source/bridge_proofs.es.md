---
lang: es
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

> Esta página es un resumen traducido y abreviado, no una traducción completa.
> La [página canónica en inglés](bridge_proofs.md) es la fuente normativa exacta
> para la gobernanza, las API, la semántica de las pruebas y los requisitos de
> publicación.

# Pruebas de puente SCCP V1 — resumen abreviado

## Alcance de la primera versión

SCCP V1 es un protocolo cerrado para la primera versión. Las únicas fuentes
externas admitidas son `ethereum-mainnet`, `bsc-mainnet` y `tron-mainnet`, y el
único destino SORA es `sora-taira`. Solana, TON, las redes personalizadas y
cualquier otro destino SORA no están admitidos y se rechazan de forma segura.

En esta versión, `SubmitBridgeProof` solo admite las pruebas tipadas
`NativeProtocol` y `SccpDestination`. El envío de pruebas genéricas `Ics` o
`TransparentZk` no está disponible y se rechaza hasta que exista un verificador
autoritativo en la cadena.

## Registro tipado y protección contra repetición

`SccpRegistryV1` es un registro tipado, vinculado a cada lane y de solo anexado
(append-only). Cada lane conserva como máximo 64 revisiones de route y 4.096
native trust anchors. Los registros históricos nunca se expulsan de forma
implícita; al alcanzar el límite, la siguiente anexión se rechaza atómicamente
sin modificar el estado.

Los intervalos de anchor usan una coordenada autenticada de progreso del
consenso: Ethereum usa el finalized beacon slot, mientras que BSC y TRON usan
la altura del native block finalizado. Un anchor anterior sigue siendo válido
hasta el checkpoint sucesor inclusive; el último anchor vigente tiene final
abierto. El finality cutoff de una route terminal debe coincidir exactamente
con el checkpoint sucesor del anchor histórico.

El registro inbound duradero conserva por separado la altura de finalidad del
evento/origen y el `anchor_interval_height` verificado. Un índice high-water
duradero, indexado por lane y hash del anchor, impide que la gobernanza elija un
checkpoint sucesor inferior a una coordenada ya admitida. La hidratación de
snapshots recalcula el índice desde los registros duraderos y exige igualdad
exacta; rechaza índices ausentes, obsoletos, malformados o sin respaldo. Los
identificadores de mensajes consumidos también se conservan para impedir el
replay.

La route de origen de TRON usa la ABI exacta
`transferToTaira(bytes,uint256,uint64 expectedNonce)`. La ejecución solo tiene
éxito cuando `expectedNonce == transferNonce`; después escribe ese mismo valor
en el canonical payload antes de incrementar el storage. La admisión native
reconstruye la llamada ABI completa a partir del recipient del payload, el
importe escalado y el nonce. Por ello, el selector retirado de dos argumentos,
un nonce antiguo o futuro y un nonce `uint64` agotado se rechazan de forma
segura.

## Verificación de una sola pasada y límites de trabajo

Las pruebas destination y native se estructuran una vez, se vinculan una vez y
reservan trabajo determinista antes de iniciar criptografía costosa. La ruta
destination verifica una sola vez el pairing-product BN254 y una sola vez la
finalidad BLS local. Las rutas native exigen el prefijo canónico más corto: el
límite es de 1.004 headers para BSC y 54 para TRON.

`[zk.sccp]` impone límites no nulos por transacción y por bloque sobre el número
y los bytes de las pruebas, los native headers/bytes, las actualizaciones del
light client de Ethereum, las recuperaciones secp256k1, las comprobaciones BLS
aggregate y sus contribuciones de claves, y las comprobaciones de pairing
BN254. Estos límites de admisión están vinculados al consenso: todos los
validadores deben usar los mismos valores del archivo de configuración y no
existen overrides mediante variables de entorno.

Los límites predeterminados de la primera versión son:

| Dimensión de trabajo | Transacción | Bloque |
|---|---:|---:|
| proofs | 1 | 4 |
| canonical proof bytes | 8 MiB | 32 MiB |
| BSC/TRON continuation headers | 1.004 | 4.016 |
| Ethereum light-client updates | 128 | 512 |
| framed native-finality bytes | 8 MiB | 32 MiB |
| secp256k1 recoveries | 1.005 | 4.020 |
| BLS aggregate checks | 1.004 | 4.016 |
| BLS key/contribution work items | 131.713 | 526.852 |
| BN254 pairing-product checks | 1 | 4 |

Una proof puede contener como máximo 8 MiB de canonical bytes. El trabajo
reservado por una transacción abandonada o rechazada no se filtra al bloque.

## Compromiso outbound, retención y descubrimiento

Cada mensaje outbound correcto recibe un `commitment_index` denso en el orden de
ejecución del bloque (`0..=511`). V1 fija como límites invariables 512 mensajes por
bloque y 4.096 bytes de payload canónico por mensaje. `[zk.sccp]` limita de forma
conjunta el payload pendiente mediante `max_pending_outbound_messages` (valor
predeterminado `65536`) y `max_pending_outbound_payload_bytes` (valor predeterminado
`268435456`).

Antes de publicar la finalidad o expulsar el cuerpo del bloque, Kura conserva de
forma inmutable el header canónico exacto y el archivo SCCP autenticado por la raíz.
La reconstrucción de proofs, bundles, proof requests e historial reciente no lee el
cuerpo histórico ni una copia mutable del payload en WSV. Al aceptar la destination
proof se eliminan atómicamente el payload pendiente y su cargo, sustituyéndolos por
un descriptor terminal de tamaño fijo sin perder el locator ni el índice. El estado
pendiente queda acotado; los registros terminales y el historial inmutable de Kura
crecen deliberadamente para conservar la protección permanente contra replay.
`GET /v1/sccp/messages/recent` usa el cursor compuesto `{ from, after_index }`.
La evidencia inmutable cuenta en el uso total/del operador del disco, pero queda
fuera del presupuesto de cuerpos expulsables.

## Límites de Torii y HTTP

Torii aplica un límite específico al cuerpo JSON de cada endpoint SCCP antes
de leer el cuerpo, asignar memoria o iniciar una verificación criptográfica.
Un `Content-Length` o cuerpo chunked que exceda el límite se rechaza con HTTP
`413`. El cliente también lee la respuesta HTTP ya decodificada bajo un límite
fijo, por lo que un `Content-Length` ausente o falso no puede eludirlo.

Todas las entradas JSON, base64 y Norito deben ser canónicas. Los campos
desconocidos, las claves duplicadas, una red/route/anchor incorrecta, el replay,
el exceso de una cuota de trabajo o una verificación fallida se rechazan sin
ninguna modificación parcial del estado.
