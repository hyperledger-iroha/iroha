---
title: Registro de curvas de cuenta
description: Mapeo canónico entre identificadores de curva del controlador de cuenta y algoritmos de firma.
---

# Registro de curvas de cuenta

Las direcciones de cuenta codifican sus controladores como un payload etiquetado
que comienza con un identificador de curva de 8 bits. Validadores, SDKs y
herramientas dependen de un registro compartido para que los identificadores de
curva se mantengan estables entre versiones y permitan una decodificación
determinista entre implementaciones.

La tabla siguiente es la referencia normativa para cada `curve_id` asignado.
Una copia legible por máquina se publica junto a este documento en
[`address_curve_registry.json`](address_curve_registry.json); el tooling
automatizado DEBERÍA consumir la versión JSON y fijar su campo `version` al
generar fixtures.

## Curvas registradas

| ID (`curve_id`) | Algoritmo | Gate de funcionalidad | Estado | Codificación de clave pública | Notas |
|-----------------|-----------|------------------------|--------|-------------------------------|-------|
| `0x01` (1) | `ed25519` | — | Producción | Clave Ed25519 comprimida de 32 bytes | Curva canónica para V1. Todas las compilaciones de SDK DEBEN admitir este identificador. |
| `0x02` (2) | `ml-dsa` | — | Producción (controlado por configuración) | Clave pública Dilithium3 (1952 bytes) | Disponible en todas las compilaciones. Habilitar en `crypto.allowed_signing` + `crypto.curves.allowed_curve_ids` antes de emitir payloads de controlador. |
| `0x03` (3) | `bls_normal` | `bls` | Producción (controlado por feature) | Clave pública G1 comprimida de 48 bytes | Requerida para validadores de consenso. La admisión permite controladores BLS incluso cuando `allowed_signing`/`allowed_curve_ids` los omiten. |
| `0x04` (4) | `secp256k1` | — | Producción | Clave SEC1 comprimida de 33 bytes | ECDSA determinista sobre SHA-256; las firmas usan el diseño canónico de 64 bytes `r∥s`. |
| `0x05` (5) | `bls_small` | `bls` | Producción (controlado por feature) | Clave pública G2 comprimida de 96 bytes | Perfil BLS de firma compacta (firmas más pequeñas, claves públicas más grandes). |
| `0x0A` (10) | `gost3410-2012-256-paramset-a` | `gost` | Reservado | Punto TC26 param set A de 64 bytes little-endian | Se habilita con la feature `gost` cuando la gobernanza apruebe el despliegue. |
| `0x0B` (11) | `gost3410-2012-256-paramset-b` | `gost` | Reservado | Punto TC26 param set B de 64 bytes little-endian | Refleja el conjunto de parámetros TC26 B; bloqueado tras la feature `gost`. |
| `0x0C` (12) | `gost3410-2012-256-paramset-c` | `gost` | Reservado | Punto TC26 param set C de 64 bytes little-endian | Reservado para aprobación futura de gobernanza. |
| `0x0D` (13) | `gost3410-2012-512-paramset-a` | `gost` | Reservado | Punto TC26 param set A de 128 bytes little-endian | Reservado en espera de demanda para curvas GOST de 512 bits. |
| `0x0E` (14) | `gost3410-2012-512-paramset-b` | `gost` | Reservado | Punto TC26 param set B de 128 bytes little-endian | Reservado en espera de demanda para curvas GOST de 512 bits. |
| `0x0F` (15) | `sm2` | `sm` | Reservado | Longitud DistID (u16 BE) + bytes DistID + clave SM2 SEC1 sin comprimir de 65 bytes | Estará disponible cuando la feature `sm` salga de preview. |

### Guía de uso

- **Fail closed:** Los codificadores DEBEN rechazar algoritmos no soportados con
  `ERR_UNSUPPORTED_ALGORITHM`. Los decodificadores DEBEN elevar `ERR_UNKNOWN_CURVE`
  para cualquier identificador no listado en este registro.
- **Feature gating:** BLS/GOST/SM2 siguen detrás de las features indicadas en
  tiempo de compilación. Los operadores deben habilitar las entradas
  correspondientes en `iroha_config.crypto.allowed_signing` y activar las
  features de compilación antes de emitir direcciones con esas curvas.
- **Excepciones de admisión:** Los controladores BLS están permitidos para
  validadores de consenso incluso cuando `allowed_signing`/`allowed_curve_ids`
  no los enumeran.
- **Paridad de config + manifest:** Use `iroha_config.crypto.allowed_curve_ids`
  (y el `ManifestCrypto.allowed_curve_ids` correspondiente) para publicar qué
  identificadores de curva acepta el clúster para controladores; la admisión
  ahora impone esta lista junto con `allowed_signing`.
- **Codificación determinista:** Las claves públicas se codifican exactamente
  como las entrega la implementación de firma (bytes comprimidos Ed25519,
  claves públicas ML‑DSA, puntos BLS comprimidos, etc.). Los SDKs deben exponer
  errores de validación antes de enviar payloads malformados.
- **Paridad de manifest:** Los manifests de génesis y los manifests de
  controladores DEBEN usar los mismos identificadores para que la admisión
  pueda rechazar controladores que excedan las capacidades del clúster.

## Anuncio de mapa de bits de capacidades

`GET /v1/node/capabilities` ahora expone la lista `allowed_curve_ids` y el
arreglo empaquetado `allowed_curve_bitmap` bajo `crypto.curves`. El bitmap es
little-endian a través de lanes de 64 bits (hasta cuatro valores para cubrir el
espacio de identificadores `u8` 0–255). El bit `i` activado significa que el
identificador de curva `i` está permitido por la política de admisión del
clúster.

- Ejemplo: `{ allowed_curve_ids: [1, 15] }` ⇒ `allowed_curve_bitmap: [32770]`
  porque `(1 << 1) | (1 << 15) = 32770`.
- Las curvas por encima de `63` activan bits en lanes posteriores. Los lanes de
  ceros al final se omiten para mantener los payloads cortos, así que una
  configuración que también habilite `curve_id = 130` emitiría
  `allowed_curve_bitmap = [32768, 0, 4]` (bits 15 y 130 activados).

Prefiera el bitmap para dashboards y comprobaciones de salud: una sola prueba de
bit responde preguntas de capacidad sin escanear el arreglo completo, mientras
que el tooling que necesita identificadores ordenados puede seguir usando
`allowed_curve_ids`. Exponer ambas vistas satisface el requisito del roadmap
**ADDR-3** de publicar bitmasks deterministas de capacidades para operadores y
SDKs.

## Lista de verificación de validación

Cada componente que ingiere controladores (Torii, admisión, codificadores de
SDK, tooling offline) debe aplicar las mismas comprobaciones deterministas antes
de aceptar un payload. Los pasos de abajo deben tratarse como lógica obligatoria
de validación:

1. **Resolver la política del clúster:** Analice el byte inicial `curve_id` del
   payload de cuenta y rechace el controlador si el identificador no está en
   `iroha_config.crypto.allowed_curve_ids` (y el espejo
   `ManifestCrypto.allowed_curve_ids`). Los controladores BLS son la excepción:
   cuando están compilados, la admisión los permite independientemente de las
   allowlists para que las claves de validadores de consenso sigan funcionando.
   Esto evita que los clústeres acepten curvas en preview que los operadores no
   hayan habilitado explícitamente.
2. **Forzar la longitud de codificación:** Compare la longitud del payload con
   el tamaño canónico del algoritmo antes de intentar descomprimir o expandir la
   clave. Rechace cualquier valor que falle la verificación de longitud para
   eliminar entradas malformadas temprano.
3. **Ejecutar la decodificación específica del algoritmo:** Use los mismos
   decodificadores canónicos que `iroha_crypto` (`ed25519_dalek`,
   `pqcrypto_mldsa`, `w3f_bls`/`blstrs`, `sm2`, los helpers TC26, etc.) para
   que todas las implementaciones compartan el mismo comportamiento de
   validación de subgrupos/puntos.
4. **Verificar tamaños de firma:** La admisión y los SDKs deben imponer las
   longitudes de firma listadas abajo y rechazar cualquier payload con una firma
   truncada o demasiado larga antes de ejecutar el verificador.

| Algoritmo | `curve_id` | Bytes de clave pública | Bytes de firma | Comprobaciones críticas |
|-----------|------------|------------------------|---------------|-------------------------|
| `ed25519` | `0x01` | 32 | 64 | Rechazar puntos comprimidos no canónicos, forzar limpieza de cofactor (sin puntos de orden pequeño) y asegurar `s < L` al validar firmas. |
| `ml-dsa` (Dilithium3) | `0x02` | 1952 | 3309 | Rechazar payloads que no tengan exactamente 1952 bytes antes de decodificar; parsear la clave pública Dilithium3 y verificar firmas usando pqcrypto-mldsa con longitudes canónicas. |
| `bls_normal` | `0x03` | 48 | 96 | Aceptar solo claves públicas G1 comprimidas canónicas y firmas G2 comprimidas; rechazar puntos identidad y codificaciones no canónicas. |
| `secp256k1` | `0x04` | 33 | 64 | Aceptar solo puntos SEC1 comprimidos; descomprimir y rechazar puntos no canónicos/invalidos, y verificar firmas usando la codificación canónica de 64 bytes `r∥s` (normalización low-`s` aplicada por el firmante). |
| `bls_small` | `0x05` | 96 | 48 | Aceptar solo claves públicas G2 comprimidas canónicas y firmas G1 comprimidas; rechazar puntos identidad y codificaciones no canónicas. |
| `gost3410-2012-256-paramset-a` | `0x0A` | 64 | 64 | Interpretar el payload como coordenadas `(x||y)` little-endian, asegurar que cada coordenada `< p`, rechazar el punto identidad y forzar limbs `r`/`s` canónicos de 32 bytes al verificar firmas. |
| `gost3410-2012-256-paramset-b` | `0x0B` | 64 | 64 | Igual validación que el conjunto A pero usando los parámetros de dominio TC26 B. |
| `gost3410-2012-256-paramset-c` | `0x0C` | 64 | 64 | Igual validación que el conjunto A pero usando los parámetros de dominio TC26 C. |
| `gost3410-2012-512-paramset-a` | `0x0D` | 128 | 128 | Interpretar `(x||y)` como limbs de 64 bytes, asegurar `< p`, rechazar el punto identidad y requerir limbs `r`/`s` de 64 bytes para firmas. |
| `gost3410-2012-512-paramset-b` | `0x0E` | 128 | 128 | Igual validación que el conjunto A pero usando los parámetros de dominio TC26 B de 512 bits. |
| `sm2` | `0x0F` | 2 + distid + 65 | 64 | Decodificar la longitud de distid (u16 BE), validar los bytes DistID, parsear el punto SEC1 sin comprimir, forzar reglas de subgrupo GM/T 0003, aplicar el DistID configurado y requerir limbs canónicos `(r, s)` según SM2. |

Cada fila se corresponde con el objeto `validation` dentro de
[`address_curve_registry.json`](address_curve_registry.json). El tooling que
consume la exportación JSON puede apoyarse en los campos `public_key_bytes`,
`signature_bytes` y `checks` para automatizar los mismos pasos de validación
descritos arriba; las codificaciones de longitud variable (por ejemplo SM2)
configuran `public_key_bytes` en null y documentan la regla de longitud en
`checks`.

## Solicitar un nuevo identificador de curva

1. Redacte la especificación del algoritmo (codificación, validación, manejo de
   errores) y asegure la aprobación de gobernanza para el despliegue.
2. Envíe un pull request actualizando este documento y
   `address_curve_registry.json`. Los nuevos identificadores deben ser únicos y
   caer dentro del rango inclusivo `0x01..=0xFE`.
3. Actualice SDKs, fixtures de Norito y documentación de operadores con el
   nuevo identificador antes de desplegar en redes de producción.
4. Coordine con los responsables de seguridad y observabilidad para asegurar
   que la telemetría, los runbooks y las políticas de admisión reflejen el
   nuevo algoritmo.
