---
lang: es
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 69c9a740261d0c367d52870fc1f48775ae48307056ba9b79d2f811e0c0849f20
source_last_modified: "2026-07-11"
translation_last_reviewed: 2026-07-11
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
