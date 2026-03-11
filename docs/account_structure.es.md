# Estructura de cuenta RFC

**Estado:** Aceptado (ADDR-1)  
**Audiencia:** Modelo de datos, Torii, Nexus, Wallet, equipos de gobernanza  
**Problemas relacionados:** Por determinar

## Resumen

Este documento describe la pila de direcciones de cuenta de envío implementada en
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) y el
herramientas complementarias. Proporciona:

- Una **dirección Iroha Base58 (I105)** con suma de verificación y orientada a humanos producida por
  `AccountAddress::to_i105` que une una cadena discriminante a la cuenta
  controlador y ofrece formas textuales deterministas compatibles con la interoperabilidad.
- Selectores de dominio para dominios predeterminados implícitos y resúmenes locales, con un
  Etiqueta selectora de registro global reservada para futuros enrutamientos respaldados por Nexus (el
  la búsqueda de registro **aún no se envía**).

## Motivación

Las billeteras y las herramientas fuera de la cadena dependen de alias de enrutamiento `alias@domain` (rejected legacy form) sin procesar. esto
tiene dos grandes inconvenientes:

1. **Sin enlace de red.** La cadena no tiene suma de comprobación ni prefijo de cadena, por lo que los usuarios
   Puede pegar una dirección de la red incorrecta sin recibir respuesta inmediata. el
   la transacción eventualmente será rechazada (desajuste de cadena) o, peor aún, tendrá éxito
   contra una cuenta no deseada si el destino existe localmente.
2. **Colisión de dominios.** Los dominios son solo espacios de nombres y se pueden reutilizar en cada uno
   cadena. Federación de servicios (custodios, puentes, flujos de trabajo entre cadenas)
   se vuelve frágil porque `finance` en la cadena A no está relacionado con `finance` en
   cadena b.

Necesitamos un formato de dirección amigable para los humanos que proteja contra errores de copiar y pegar
y un mapeo determinista del nombre de dominio a la cadena autorizada.

## Metas

- Describir la envolvente I105 Base58 implementada en el modelo de datos y el
  reglas canónicas de análisis/alias que siguen `AccountId` y `AccountAddress`.
- Codificar el discriminante de cadena configurado directamente en cada dirección y
  definir su proceso de gobernanza/registro.
- Describir cómo introducir un registro de dominio global sin romper con la corriente.
  implementaciones y especificar reglas de normalización/anti-spoofing.

## Sin objetivos

- Implementación de transferencias de activos entre cadenas. La capa de enrutamiento solo devuelve el
  cadena objetivo.
- Finalizar la gobernanza para la emisión de dominios globales. Este RFC se centra en los datos.
  Primitivas de modelo y transporte.

## Antecedentes

### Alias de enrutamiento actual

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical I105 literal (no `@domain` suffix)
Parse accepts:
- Encoded account identifiers only: I105.
- Runtime parsers reject canonical hex (`0x...`), any `@<domain>` suffix, and alias literals such as `label@domain`.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` vive fuera de `AccountId`. Los nodos verifican el `ChainId` de la transacción
contra configuración durante la admisión (`AcceptTransactionFail::ChainIdMismatch`)
y rechazar transacciones extranjeras, pero la cadena de cuenta en sí no lleva
pista de red.

### Identificadores de dominio

`DomainId` envuelve un `Name` (cadena normalizada) y tiene como alcance la cadena local.
Cada cadena puede registrar `wonderland`, `finance`, etc. de forma independiente.

### Contexto nexo

Nexus es responsable de la coordinación entre componentes (carriles/espacios de datos). eso
Actualmente no tiene ningún concepto de enrutamiento de dominios entre cadenas.

## Diseño propuesto

### 1. Discriminante de cadena determinista

`iroha_config::parameters::actual::Common` ahora expone:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **Restricciones:**
  - Único por red activa; gestionado a través de un registro público firmado con
    rangos reservados explícitos (por ejemplo, `0x0000–0x0FFF` prueba/dev, `0x1000–0x7FFF`
    asignaciones comunitarias, `0x8000–0xFFEF` aprobadas por la gobernanza, `0xFFF0–0xFFFF`
    reservado).
  - Inmutable para una cadena en funcionamiento. Cambiarlo requiere un hard fork y un
    actualización del registro.
- **Gobernanza y registro (planificado):** Un conjunto de gobernanza de firmas múltiples
  mantener un registro JSON firmado que mapee los discriminantes con alias humanos y
  Identificadores CAIP-2. Este registro aún no forma parte del tiempo de ejecución enviado.
- **Uso:** A través de la admisión estatal, Torii, SDK y API de billetera,
  cada componente puede incrustarlo o validarlo. La exposición al CAIP-2 sigue siendo un futuro
  tarea de interoperabilidad.

### 2. Códecs de direcciones canónicas

El modelo de datos de Rust expone una única representación de carga útil canónica
(`AccountAddress`) que se puede emitir en varios formatos orientados a humanos. I105 es
el formato de cuenta preferido para compartir y salida canónica; el comprimido
El formulario `sora` es la segunda mejor opción, exclusiva de Sora para UX, donde el alfabeto kana
agrega valor. El hexadecimal canónico sigue siendo una ayuda para la depuración.

- **I105 (Iroha Base58)** – una envoltura Base58 que incrusta la cadena
  discriminante. Los decodificadores validan el prefijo antes de promover la carga útil a
  la forma canónica.
- **Vista comprimida de Sora**: un alfabeto exclusivo de Sora de **105 símbolos** creado por
  añadiendo el poema イロハ de medio ancho (incluidos ヰ y ヱ) al poema de 58 caracteres
  Conjunto I105. Las cadenas comienzan con el centinela `sora`, incorporan un derivado de Bech32m
  suma de comprobación y omitir el prefijo de red (Sora Nexus está implícito en el centinela).

```
  I105  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Canonical hexadecimal**: una codificación `0x…` fácil de depurar del byte canónico
  sobre.

`AccountAddress::parse_encoded` detecta automáticamente I105 (preferido), comprimido (`sora`, segundo mejor) o hexadecimal canónico
(`0x...` únicamente; se rechaza el hexadecimal básico) ingresa y devuelve tanto la carga útil decodificada como la detectada.
`AccountAddress`. Torii ahora llama `parse_encoded` para ISO 20022 suplementario
aborda y almacena la forma hexadecimal canónica para que los metadatos sigan siendo deterministas
independientemente de la representación original.

#### 2.1 Diseño de bytes de encabezado (ADDR-1a)

Cada carga útil canónica se presenta como `header · controller`. el
`header` es un único byte que comunica qué reglas del analizador se aplican a los bytes que
seguir:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

Por lo tanto, el primer byte contiene los metadatos del esquema para los decodificadores posteriores:

| Puntas | Campo | Valores permitidos | Error al violar |
|------|-------|----------|--------------------|
| 7-5 | `addr_version` | `0` (v1). Los valores `1-7` están reservados para futuras revisiones. | Los valores fuera de `0-7` activan `AccountAddressError::InvalidHeaderVersion`; las implementaciones DEBEN tratar las versiones distintas de cero como no compatibles en la actualidad. |
| 4-3 | `addr_class` | `0` = clave única, `1` = multifirma. | Otros valores aumentan `AccountAddressError::UnknownAddressClass`. |
| 2-1 | `norm_version` | `1` (Norma v1). Los valores `0`, `2`, `3` están reservados. | Los valores fuera de `0-3` aumentan `AccountAddressError::InvalidNormVersion`. |
| 0 | `ext_flag` | DEBE ser `0`. | El bit establecido aumenta `AccountAddressError::UnexpectedExtensionFlag`. |

El codificador Rust escribe `0x02` para controladores de una sola tecla (versión 0, clase 0,
norma v1, indicador de extensión borrado) y `0x0A` para controladores multifirma (versión 0,
clase 1, norma v1, bandera de extensión borrada).

#### 2.2 Domainless payload semantics

Canonical payload bytes are domainless: the wire layout is `header · controller`
with no selector segment, no implicit default-domain reconstruction, and no
public decode fallback for legacy scoped-account literals.

Explicit domain context is modeled separately as `ScopedAccountId { account,
domain }` or separate API fields; it is not encoded into `AccountId` payload
bytes.

| Tag | Meaning | Payload | Notes |
|-----|---------|---------|-------|
| `0x00` | Domainless canonical scope | none | Canonical account payloads are domainless; explicit domain context lives outside the address payload. |
| `0x01` | Local domain digest | 12 bytes | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Global registry entry | 4 bytes | Big-endian `registry_id`; reserved until the global registry ships. |

Domain labels are canonicalised (UTS-46 + STD3 + NFC) before hashing. Unknown tags raise `AccountAddressError::UnknownDomainTag`. When validating an address against a domain, mismatched selectors raise `AccountAddressError::DomainMismatch`.

```
legacy selector segment
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

When present, the selector is immediately adjacent to the controller payload, so
a decoder can walk the wire format in order: read the tag byte, read the
tag-specific payload, then move on to the controller bytes.

**Legacy selector examples**

- *Implicit default* (`tag = 0x00`). No payload. Example canonical hex for the default
  domain using the deterministic test key:
  `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.
- *Local digest* (`tag = 0x01`). Payload is the 12-byte digest. Example (`treasury` seed
  `0x01`): `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.
- *Global registry* (`tag = 0x02`). Payload is a big-endian `registry_id:u32`. The bytes
  that follow the payload are identical to the implicit-default case; the selector simply
  replaces the normalised domain string with a registry pointer. Example using
  `registry_id = 0x0000_002A` (decimal 42) and the deterministic default controller:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.

#### 2.3 Codificaciones de carga útil del controlador (ADDR-1a)

La carga útil del controlador es otra unión etiquetada que se agrega después del selector de dominio:

| Etiqueta | Controlador | Diseño | Notas |
|-----|------------|--------|-------|
| `0x00` | Tecla única | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` se asigna a Ed25519 hoy. `key_len` está limitado a `u8`; los valores más grandes generan `AccountAddressError::KeyPayloadTooLong` (por lo que las claves públicas ML‑DSA de clave única, que tienen >255 bytes, no se pueden codificar y deben usar multifirma). |
| `0x01` | Multifirma | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | Admite hasta 255 miembros (`CONTROLLER_MULTISIG_MEMBER_MAX`). Curvas desconocidas elevan `AccountAddressError::UnknownCurve`; Las políticas mal formadas surgen como `AccountAddressError::InvalidMultisigPolicy`. |

Las políticas multifirma también exponen un mapa CBOR estilo CTAP2 y un resumen canónico para que
Los hosts y SDK pueden verificar el controlador de forma determinista. Ver
`docs/source/references/multisig_policy_schema.md` (ADDR-1c) para el esquema,
reglas de validación, procedimiento hash y accesorios dorados.

Todos los bytes clave están codificados exactamente como los devuelve `PublicKey::to_bytes`; los decodificadores reconstruyen instancias `PublicKey` y generan `AccountAddressError::InvalidPublicKey` si los bytes no coinciden con la curva declarada.

> **Ed25519 aplicación canónica (ADDR-3a):** las claves curvas `0x01` deben decodificarse en la cadena de bytes exacta emitida por el firmante y no deben estar en el subgrupo de orden pequeño. Los nodos ahora rechazan codificaciones no canónicas (por ejemplo, valores reducidos en módulo `2^255-19`) y puntos débiles como el elemento de identidad, por lo que los SDK deberían mostrar errores de validación de coincidencias antes de enviar direcciones.

##### 2.3.1 Registro de identificadores de curvas (ADDR-1d)

| ID (`curve_id`) | Algoritmo | Puerta característica | Notas |
|-----------------|-----------|--------------|-------|
| `0x00` | Reservado | — | NO DEBE emitirse; superficie de decodificadores `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519 | — | Algoritmo canónico v1 (`Algorithm::Ed25519`); habilitado en la configuración predeterminada. |
| `0x02` | ML‑DSA (Dilitio3) | — | Utiliza los bytes de clave pública Dilithium3 (1952 bytes). Las direcciones de clave única no pueden codificar ML‑DSA porque `key_len` es `u8`; multisig utiliza longitudes `u16`. |
| `0x03` | BLS12‑381 (normal) | `bls` | Claves públicas en G1 (48 bytes), firmas en G2 (96 bytes). |
| `0x04` | secp256k1 | — | ECDSA determinista sobre SHA‑256; las claves públicas utilizan el formato comprimido SEC1 de 33 bytes y las firmas utilizan el diseño canónico `r∥s` de 64 bytes. |
| `0x05` | BLS12‑381 (pequeño) | `bls` | Claves públicas en G2 (96 bytes), firmas en G1 (48 bytes). |
| `0x0A` | GOST R 34.10‑2012 (256, conjunto A) | `gost` | Disponible solo cuando la función `gost` está habilitada. |
| `0x0B` | GOST R 34.10‑2012 (256, conjunto B) | `gost` | Disponible solo cuando la función `gost` está habilitada. |
| `0x0C` | GOST R 34.10‑2012 (256, conjunto C) | `gost` | Disponible solo cuando la función `gost` está habilitada. |
| `0x0D` | GOST R 34.10‑2012 (512, conjunto A) | `gost` | Disponible solo cuando la función `gost` está habilitada. |
| `0x0E` | GOST R 34.10‑2012 (512, conjunto B) | `gost` | Disponible solo cuando la función `gost` está habilitada. |
| `0x0F` | SM2 | `sm` | Longitud de DistID (u16 BE) + bytes de DistID + clave SM2 sin comprimir SEC1 de 65 bytes; disponible solo cuando `sm` está habilitado. |

Las ranuras `0x06–0x09` permanecen sin asignar para curvas adicionales; presentando un nuevo
El algoritmo requiere una actualización de la hoja de ruta y una cobertura de SDK/host coincidente. Codificadores
DEBE rechazar cualquier algoritmo no compatible con `ERR_UNSUPPORTED_ALGORITHM`, y
los decodificadores DEBEN fallar rápidamente en identificadores desconocidos con `ERR_UNKNOWN_CURVE` para preservar
Comportamiento cerrado ante fallos.

El registro canónico (incluida una exportación JSON legible por máquina) se encuentra en
[`docs/source/references/address_curve_registry.md`](fuente/referencias/address_curve_registry.md).
Las herramientas DEBEN consumir ese conjunto de datos directamente para que los identificadores de curva permanezcan
coherente entre los SDK y los flujos de trabajo del operador.

- **Gating de SDK:** Los SDK tienen de forma predeterminada validación/codificación exclusiva para Ed25519. Swift expone
  indicadores en tiempo de compilación (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); el SDK de Java/Android requiere
  `AccountAddress.configureCurveSupport(...)`; el SDK de JavaScript utiliza
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  La compatibilidad con secp256k1 está disponible pero no está habilitada de forma predeterminada en JS/Android
  SDK; las personas que llaman deben optar por participar explícitamente cuando emiten controladores que no sean Ed25519.
- **Gating de host:** `Register<Account>` rechaza los controladores cuyos firmantes utilizan algoritmos
  falta en la lista `crypto.allowed_signing` del nodo **o** identificadores de curva ausentes en
  `crypto.curves.allowed_curve_ids`, por lo que los clústeres deben anunciar soporte (configuración +
  genesis) antes de que se puedan registrar los controladores ML‑DSA/GOST/SM. Controlador BLS
  los algoritmos siempre están permitidos cuando se compilan (las claves de consenso dependen de ellos),
  y la configuración predeterminada habilita Ed25519 + secp256k1.【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Guía del controlador multifirma

`AccountController::Multisig` serializa políticas a través de
`crates/iroha_data_model/src/account/controller.rs` y aplica el esquema
documentado en [`docs/source/references/multisig_policy_schema.md`](fuente/referencias/multisig_policy_schema.md).
Detalles clave de implementación:

- Las políticas son normalizadas y validadas por `MultisigPolicy::validate()` antes
  estando incrustado. Los umbrales deben ser ≥1 y ≤Σ peso; los miembros duplicados son
  eliminado de forma determinista después de ordenar por `(algorithm || 0x00 || key_bytes)`.
- La carga útil del controlador binario (`ControllerPayload::Multisig`) codifica
  `version:u8`, `threshold:u16`, `member_count:u8`, luego el de cada miembro.
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. Esto es exactamente lo que
  `AccountAddress::canonical_bytes()` escribe en cargas útiles I105 (preferida)/sora (segunda mejor).
- Hashing (`MultisigPolicy::digest_blake2b256()`) usa Blake2b-256 con el
  `iroha-ms-policy` cadena de personalización para que los manifiestos de gobernanza puedan vincularse a un
  ID de política determinista que coincide con los bytes del controlador integrados en I105.
- La cobertura de accesorios vive en `fixtures/account/address_vectors.json` (casos
  `addr-multisig-*`). Las carteras y los SDK deben hacer valer las cadenas canónicas I105
  a continuación para confirmar que sus codificadores coincidan con la implementación de Rust.

| Identificación del caso | Umbral / miembros | Literal I105 (prefijo `0x02F1`) | Sora comprimido (`sora`) literal | Notas |
|---------|---------------------|--------------------------------|-------------------------|-------|
| `addr-multisig-council-threshold3` | `≥3` peso, miembros `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | Quórum de gobernanza en el ámbito del consejo. |
| `addr-multisig-wonderland-threshold2` | `≥2`, miembros `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | Ejemplo de país de las maravillas de doble firma (peso 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`, miembros `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | Quórum de dominio implícito predeterminado utilizado para la gobernanza base.

#### 2.4 Reglas de falla (ADDR-1a)

- Las cargas útiles más cortas que el encabezado + selector requerido o con bytes sobrantes emiten `AccountAddressError::InvalidLength` o `AccountAddressError::UnexpectedTrailingBytes`.
- Los encabezados que establezcan el `ext_flag` reservado o anuncien versiones/clases no compatibles DEBEN rechazarse usando `UnexpectedExtensionFlag`, `InvalidHeaderVersion` o `UnknownAddressClass`.
- Las etiquetas de selector/controlador desconocidas generan `UnknownDomainTag` o `UnknownControllerTag`.
- El material clave sobredimensionado o mal formado genera `KeyPayloadTooLong` o `InvalidPublicKey`.
- Los controladores multifirma que superan los 255 miembros generan `MultisigMemberOverflow`.
- Conversiones IME/NFKC: Sora kana de ancho medio se puede normalizar a sus formas de ancho completo sin interrumpir la decodificación, pero el centinela ASCII `sora` y los dígitos/letras I105 DEBEN permanecer ASCII. Los centinelas de ancho completo o plegados en mayúsculas aparecen `ERR_MISSING_COMPRESSED_SENTINEL`, las cargas útiles ASCII de ancho completo aumentan `ERR_INVALID_COMPRESSED_CHAR` y las discrepancias en las sumas de comprobación aparecen como `ERR_CHECKSUM_MISMATCH`. Las pruebas de propiedad en `crates/iroha_data_model/src/account/address.rs` cubren estas rutas para que los SDK y las billeteras puedan depender de fallas deterministas.
- El análisis de Torii y SDK de los alias `address@domain` (rejected legacy form) ahora emite los mismos códigos `ERR_*` cuando las entradas I105 (preferida)/sora (segundo mejor) fallan antes de la reserva de alias (por ejemplo, falta de coincidencia en la suma de comprobación, falta de coincidencia en el resumen del dominio), para que los clientes puedan transmitir razones estructuradas sin tener que adivinar a partir de cadenas en prosa.
- Las cargas útiles del selector local de menos de 12 bytes aparecen en `ERR_LOCAL8_DEPRECATED`, lo que preserva una transferencia directa de los resúmenes heredados de Local-8.
- Domainless canonical I105 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 Vectores binarios normativos

- **Dominio predeterminado implícito (`default`, byte inicial `0x00`)**  
  Hexadecimal canónico: `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.  
  Desglose: `0x02` encabezado, `0x00` selector (valor predeterminado implícito), `0x00` etiqueta del controlador, `0x01` ID de curva (Ed25519), `0x20` longitud de la clave, seguido de la carga útil de la clave de 32 bytes.
- **Resumen del dominio local (`treasury`, byte inicial `0x01`)**  
  Hexadecimal canónico: `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.  
  Desglose: encabezado `0x02`, etiqueta de selector `0x01` más resumen `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, seguido de la carga útil de una sola clave (`0x00` etiqueta, `0x01` ID de curva, `0x20` longitud, clave Ed25519 de 32 bytes).

Las pruebas unitarias (`account::address::tests::parse_encoded_accepts_all_formats`) afirman los vectores V1 a continuación a través de `AccountAddress::parse_encoded`, lo que garantiza que las herramientas puedan confiar en la carga útil canónica en formas hexadecimales, I105 (preferida) y comprimidas (`sora`, segunda mejor). Regenere el conjunto de accesorios extendido con `cargo run -p iroha_data_model --example address_vectors`.

| Dominio | Byte semilla | Hexágono canónico | Comprimido (`sora`) |
|-------------|-----------|-------------------------------------------------------------------------------|------------|
| predeterminado | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| tesorería | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| país de las maravillas | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| iroha | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| alfa | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| omega | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| gobernanza | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| validadores | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| explorador | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| soranet | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| kitsune | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| da | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

Revisado por: Grupo de trabajo sobre modelos de datos, Grupo de trabajo sobre criptografía: alcance aprobado para ADDR-1a.

##### Alias ​​de referencia de Sora Nexus

Las redes de Sora Nexus están predeterminadas en `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). el
Por lo tanto, los ayudantes `AccountAddress::to_i105` y `to_i105` emiten
formas textuales consistentes para cada carga útil canónica. Accesorios seleccionados de
`fixtures/account/address_vectors.json` (generado a través de
`cargo xtask address-vectors`) se muestran a continuación para una referencia rápida:

| Cuenta/selector | Literal I105 (prefijo `0x02F1`) | Sora comprimido (`sora`) literal |
|--------------------|--------------------------------|-------------------------|
| dominio `default` (selector implícito, semilla `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (sufijo `@default` opcional al proporcionar sugerencias de enrutamiento explícitas) |
| `treasury` (selector de resumen local, semilla `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| Puntero de registro global (`registry_id = 0x0000_002A`, equivalente a `treasury`) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

Estas cadenas coinciden con las emitidas por la CLI (`iroha tools address convert`), Torii
respuestas (`canonical I105 literal rendering`) y ayudantes de SDK, para que UX copie y pegue
Los flujos pueden confiar en ellos palabra por palabra. Agregue `<address>@<domain>` (rejected legacy form) solo cuando necesite una sugerencia de enrutamiento explícita; el sufijo no forma parte de la salida canónica.

#### 2.6 Alias textuales para la interoperabilidad (previsto)

- **Estilo de alias de cadena:** `ih:<chain-alias>:<alias@domain>` para registros y humanos
  entrada. Las billeteras deben analizar el prefijo, verificar la cadena integrada y bloquear
  desajustes.
- **formulario CAIP-10:** `iroha:<caip-2-id>:<i105-addr>` para cadena independiente
  integraciones. Esta asignación **aún no está implementada** en el paquete enviado.
  cadenas de herramientas.
- **Ayudantes de máquina:** Publicar códecs para Rust, TypeScript/JavaScript, Python,
  y Kotlin que cubre I105 y formatos comprimidos (`AccountAddress::to_i105`,
  `AccountAddress::parse_encoded` y sus equivalentes de SDK). Los ayudantes del CAIP-10 son
  trabajo futuro.

#### 2.7 Alias ​​determinista I105

- **Asignación de prefijos:** Reutilice `chain_discriminant` como prefijo de red I105.
  `encode_i105_prefix()` (ver `crates/iroha_data_model/src/account/address.rs`)
  emite un prefijo de 6 bits (un solo byte) para los valores `<64` y un prefijo de 14 bits de dos bytes
  formulario para redes más grandes. Las asignaciones autorizadas viven en
  [`address_prefix_registry.md`](fuente/referencias/address_prefix_registry.md);
  Los SDK DEBEN mantener sincronizado el registro JSON coincidente para evitar colisiones.
- **Material de cuenta:** I105 codifica la carga útil canónica creada por
  `AccountAddress::canonical_bytes()`: byte de encabezado, selector de dominio y
  carga útil del controlador. No hay ningún paso de hash adicional; I105 incorpora el
  Carga útil del controlador binario (clave única o multifirma) producida por Rust
  codificador, no el mapa CTAP2 utilizado para resúmenes de políticas multifirma.
- **Codificación:** `encode_i105()` concatena los bytes del prefijo con el canónico
  carga útil y agrega una suma de verificación de 16 bits derivada de Blake2b-512 con el valor fijo
  prefijo `I105PRE` (`b"I105PRE" || prefix || payload`). El resultado está codificado en Base58 mediante `bs58`.
  Los ayudantes de CLI/SDK exponen el mismo procedimiento y `AccountAddress::parse_encoded`
  lo invierte a través de `decode_i105`.

#### 2.8 Vectores de prueba textuales normativos

`fixtures/account/address_vectors.json` contiene I105 completo (preferido) y comprimido (`sora`, segundo mejor)
literales para cada carga útil canónica. Aspectos destacados:

- **`addr-single-default-ed25519` (Sora Nexus, prefijo `0x02F1`).**  
  I105 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`, comprimido (`sora`)
  `sora2QG…U4N5E5`. Torii emite estas cadenas exactas de `AccountId`
  Implementación `Display` (canónica I105) y `AccountAddress::to_i105`.
- **`addr-global-registry-002a` (selector de registro → tesorería).**  
  I105 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`, comprimido (`sora`)
  `sorakX…CM6AEP`. Demuestra que los selectores de registro todavía decodifican a
  la misma carga útil canónica que el resumen local correspondiente.
- **Caso de falla (`i105-prefix-mismatch`).**  
  Análisis de un literal I105 codificado con el prefijo `NETWORK_PREFIX + 1` en un nodo
  esperando que el prefijo predeterminado produzca
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  antes de intentar el enrutamiento del dominio. El accesorio `i105-checksum-mismatch`
  ejerce la detección de manipulación sobre la suma de comprobación Blake2b.

#### 2.9 Elementos de cumplimiento

ADDR‑2 envía un paquete de accesorios rejugables que cubren aspectos positivos y negativos.
escenarios en hexadecimal canónico, I105 (preferido), comprimido (`sora`, ancho medio/completo), implícito
selectores predeterminados, alias de registro global y controladores multifirma. el
JSON canónico vive en `fixtures/account/address_vectors.json` y puede ser
regenerado con:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

Para experimentos ad hoc (diferentes rutas/formatos), el binario de ejemplo sigue siendo
disponible:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

Pruebas unitarias de óxido en `crates/iroha_data_model/tests/account_address_vectors.rs`
y `crates/iroha_torii/tests/account_address_vectors.rs`, junto con el JS,
Arneses Swift y Android (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`,
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
Consuma el mismo dispositivo para garantizar la paridad de códecs entre los SDK y la admisión Torii.

### 3. Dominios globalmente únicos y normalización

Ver también: [`docs/source/references/address_norm_v1.md`](fuente/referencias/address_norm_v1.md)
para la canalización canónica de Norm v1 utilizada en Torii, el modelo de datos y los SDK.

Redefina `DomainId` como una tupla etiquetada:

```
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // new enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // default for the local chain
    External { chain_discriminant: u16 },
}
```

`LocalChain` envuelve el nombre existente para los dominios administrados por la cadena actual.
Cuando un dominio se registra a través del registro global, persistimos en la propiedad
discriminante de la cadena. La visualización/análisis permanece sin cambios por ahora, pero el
La estructura ampliada permite decisiones de enrutamiento.

#### 3.1 Normalización y defensas contra la suplantación de identidad

La norma v1 define la canalización canónica que cada componente debe usar antes de un dominio.
El nombre persiste o está incrustado en un `AccountAddress`. El tutorial completo
vive en [`docs/source/references/address_norm_v1.md`](fuente/referencias/address_norm_v1.md);
El siguiente resumen captura los pasos que las billeteras, Torii, SDK y gobernanza
herramientas deben implementar.

1. **Validación de entrada.** Rechazar cadenas vacías, espacios en blanco y reservados
   delimitadores `@`, `#`, `$`. Esto coincide con las invariantes impuestas por
   `Name::validate_str`.
2. **Composición NFC Unicode.** Aplique la normalización NFC respaldada por ICU de manera canónica
   las secuencias equivalentes colapsan de forma determinista (por ejemplo, `e\u{0301}` → `é`).
3. **Normalización UTS-46.** Ejecute la salida NFC a través de UTS‑46 con
   `use_std3_ascii_rules = true`, `transitional_processing = false`, y
   Aplicación de longitud de DNS habilitada. El resultado es una secuencia de etiqueta A minúscula;
   las entradas que violan las reglas STD3 fallan aquí.
4. **Límites de longitud.** Aplique los límites de estilo DNS: cada etiqueta DEBE tener entre 1 y 63
   bytes y el dominio completo NO DEBE exceder los 255 bytes después del paso 3.
5. **Política de confusión opcional.** Se realiza un seguimiento de las verificaciones de secuencias de comandos UTS-39 para
   Norma v2; Los operadores pueden habilitarlos temprano, pero si falla la verificación deben abortar.
   procesamiento.

Si cada etapa tiene éxito, la cadena de etiqueta A en minúscula se almacena en caché y se utiliza para
codificación de direcciones, configuración, manifiestos y búsquedas de registro. resumen local
los selectores derivan su valor de 12 bytes como `blake2s_mac(key = "SORA-LOCAL-K:v1",
canonical_label)[0..12]` usando la salida del paso 3. Todos los demás intentos (mixtos
mayúsculas, mayúsculas, entrada Unicode sin formato) se rechazan con estructura
`ParseError`s en el límite donde se proporcionó el nombre.

Accesorios canónicos que demuestran estas reglas, incluidos los viajes de ida y vuelta de punycode
y secuencias STD3 no válidas, se enumeran en
`docs/source/references/address_norm_v1.md` y se reflejan en el SDK CI
conjuntos de vectores rastreados bajo ADDR-2.

### 4. Registro y enrutamiento del dominio Nexus

- **Esquema de registro:** Nexus mantiene un mapa firmado `DomainName -> ChainRecord`
  donde `ChainRecord` incluye la cadena discriminante, metadatos opcionales (RPC
  puntos finales) y una prueba de autoridad (por ejemplo, firma múltiple de gobernanza).
- **Mecanismo de sincronización:**
  - Las cadenas envían reclamos de dominio firmados a Nexus (ya sea durante la génesis o mediante
    instrucción de gobernanza).
  - Nexus publica manifiestos periódicos (JSON firmado más raíz Merkle opcional)
    a través de HTTPS y almacenamiento dirigido a contenido (por ejemplo, IPFS). Los clientes fijan el
    último manifiesto y verificar firmas.
- **Flujo de búsqueda:**
  - Torii recibe una transacción que hace referencia a `DomainId`.
  - Si el dominio es desconocido localmente, Torii consulta el manifiesto Nexus almacenado en caché.
  - Si el manifiesto indica cadena extranjera, la transacción se rechaza con
    un error determinista `ForeignDomain` y la información de la cadena remota.
  - Si falta el dominio en Nexus, Torii devuelve `UnknownDomain`.
- **Anclajes de confianza y rotación:** Las claves de gobernanza firman manifiestos; rotación o
  La revocación se publica como una nueva entrada del manifiesto. Los clientes hacen cumplir el manifiesto
  TTL (por ejemplo, 24 horas) y se niegan a consultar datos obsoletos más allá de esa ventana.
- **Modos de falla:** Si falla la recuperación del manifiesto, Torii vuelve al caché
  datos dentro de TTL; pasado TTL emite `RegistryUnavailable` y se niega
  enrutamiento entre dominios para evitar estados inconsistentes.

### 4.1 Inmutabilidad del registro, alias y lápidas (ADDR-7c)

Nexus publica un **manifiesto de solo agregar** para que cada asignación de dominio o alias
puede ser auditado y reproducido. Los operadores deben tratar el paquete descrito en el
[runbook de manifiesto de direcciones](fuente/runbooks/address_manifest_ops.md) como el
única fuente de verdad: si falta un manifiesto o no pasa la validación, Torii debe
negarse a resolver el dominio afectado.

Soporte de automatización: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
reproduce las comprobaciones de suma de comprobación, esquema y resumen anterior detalladas en el
libro de ejecución. Incluya el resultado del comando en los tickets de cambio para mostrar el `sequence`
y el enlace `previous_digest` se validó antes de publicar el paquete.

#### Encabezado del manifiesto y contrato de firma

| Campo | Requisito |
|-------|-------------|
| `version` | Actualmente `1`. Mejora solo con una actualización de especificaciones coincidente. |
| `sequence` | Incrementar **exactamente** uno por publicación. Los cachés Torii rechazan revisiones con lagunas o regresiones. |
| `generated_ms` + `ttl_hours` | Establecer la actualización de la caché (predeterminado 24 h). Si el TTL expira antes de la próxima publicación, Torii cambia a `RegistryUnavailable`. |
| `previous_digest` | Resumen BLAKE3 (hexadecimal) del cuerpo manifiesto anterior. Los verificadores lo recalculan con `b3sum` para demostrar la inmutabilidad. |
| `signatures` | Los manifiestos se firman a través de Sigstore (`cosign sign-blob`). Las operaciones deben ejecutar `cosign verify-blob --bundle manifest.sigstore manifest.json` y hacer cumplir las restricciones de identidad/emisor de gobernanza antes de la implementación. |

La automatización de lanzamiento emite `manifest.sigstore` y `checksums.sha256`
junto al cuerpo JSON. Mantenga los archivos juntos al duplicar en SoraFS o
Puntos finales HTTP para que los auditores puedan reproducir los pasos de verificación palabra por palabra.

#### Tipos de entrada

| Tipo | Propósito | Campos obligatorios |
|------|---------|-----------------|
| `global_domain` | Declara que un dominio está registrado globalmente y debe asignarse a una cadena discriminante y al prefijo I105. | `{ "domain": "<label>", "chain": "sora:nexus:global", "i105_prefix": 753, "selector": "global" }` |
| `tombstone` | Retira un alias/selector de forma permanente. Requerido al borrar resúmenes de Local-8 o eliminar un dominio. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

Las entradas `global_domain` pueden incluir opcionalmente un `manifest_url` o `sorafs_cid`
apuntar billeteras a metadatos de cadena firmados, pero la tupla canónica permanece
`{domain, chain, discriminant/i105_prefix}`. `tombstone` registros **deben** citar
el selector que se retira y el boleto/artefacto de gobierno que autorizó
el cambio para que la pista de auditoría se pueda reconstruir fuera de línea.

#### Alias/flujo de trabajo y telemetría

1. **Detectar deriva.** Utilice `torii_address_local8_total{endpoint}`,
   `torii_address_local8_domain_total{endpoint,domain}`,
   `torii_address_collision_total{endpoint,kind="local12_digest"}`,
   `torii_address_collision_domain_total{endpoint,domain}`,
   `torii_address_domain_total{endpoint,domain_kind}`, y
   `torii_address_invalid_total{endpoint,reason}` (representado en
   `dashboards/grafana/address_ingest.json`) para confirmar los envíos locales y
   Las colisiones del Local-12 se mantienen en cero antes de proponer una lápida. el
   Los contadores por dominio permiten a los propietarios demostrar que solo los dominios de desarrollo/prueba emiten Local-8.
   tráfico (y que las colisiones Local-12 se asignan a dominios intermedios conocidos) mientras
   incluye el panel **Domain Kind Mix (5m)** para que SRE pueda graficar cuánto
   El tráfico `domain_kind="local12"` permanece y el `AddressLocal12Traffic`
   La alerta se activa cada vez que la producción todavía ve selectores Local-12 a pesar de la
   puerta de retiro.
2. **Derivar resúmenes canónicos.** Ejecutar
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (o consumir `fixtures/account/address_vectors.json` a través de
   `scripts/account_fixture_helper.py`) para capturar el `digest_hex` exacto.
   La CLI acepta literales I105, `i105` y canónicos `0x…`; anexar
   `@<domain>` solo cuando necesite conservar una etiqueta para los manifiestos.
   El resumen JSON muestra ese dominio a través del campo `input_domain`, y
   `legacy  suffix` reproduce la codificación convertida como `<address>@<domain>` (rejected legacy form) para
   diferencias de manifiesto (este sufijo son metadatos, no una identificación de cuenta canónica).
   Para exportaciones orientadas a nueva línea, utilice
   `iroha tools address normalize --input <file> legacy-selector input mode` para convertir local en masa
   selectores en formatos canónicos I105 (preferido), comprimido (`sora`, segundo mejor), hexadecimal o JSON mientras se omite
   filas no locales. Cuando los auditores necesiten evidencia compatible con hojas de cálculo, ejecute
   `iroha tools address audit --input <file> --format csv` para emitir un resumen CSV
   (`input,status,format,domain_kind,…`) que resalta los selectores locales,
   codificaciones canónicas y errores de análisis en el mismo archivo.
3. **Agregar entradas de manifiesto.** Redacte el registro `tombstone` (y el registro
   `global_domain` registro al migrar al registro global) y validar
   el manifiesto con `cargo xtask address-vectors` antes de solicitar firmas.
4. **Verificar y publicar.** Siga la lista de verificación del runbook (hashes, Sigstore,
   monotonicidad de la secuencia) antes de reflejar el paquete en SoraFS. Torii ahora
   canonicaliza los literales I105 (preferido)/sora (segundo mejor) inmediatamente después de que llega el paquete.
5. **Monitorizar y revertir.** Mantenga los paneles de colisión Local‑8 y Local‑12 en
   cero por 30 días; si aparecen regresiones, vuelva a publicar el manifiesto anterior
   solo en el entorno de no producción afectado hasta que la telemetría se estabilice.

Todos los pasos anteriores son evidencia obligatoria para ADDR-7c: manifiestos sin
el paquete de firmas `cosign` o sin valores `previous_digest` coincidentes deben
ser rechazado automáticamente, y los operadores deben adjuntar los registros de verificación a
sus billetes de cambio.

### 5. Ergonomía de billetera y API

- **Valores predeterminados de visualización:** Las billeteras muestran la dirección I105 (breve, con suma de verificación)
  más el dominio resuelto como una etiqueta extraída del registro. Los dominios son
  claramente marcados como metadatos descriptivos que pueden cambiar, mientras que I105 es el
  dirección estable.
- **Canonicalización de entrada:** Torii y SDK aceptan I105 (preferido)/sora (segundo mejor)/0x
  direcciones más `alias@domain` (rejected legacy form), `uaid:…`, y
  `opaque:…` formularios, luego canonicalizarlos a I105 para su salida. no hay
  alternancia de modo estricto; Los identificadores de teléfono/correo electrónico sin procesar deben mantenerse fuera del libro mayor.
  a través de UAID/mapeos opacos.
- **Prevención de errores:** Las billeteras analizan los prefijos I105 y aplican la discriminación en cadena
  expectativas. Los desajustes en la cadena provocan fallas graves con diagnósticos procesables.
- **Bibliotecas de códecs:** Rust oficial, TypeScript/JavaScript, Python y Kotlin
  Las bibliotecas proporcionan codificación/decodificación I105 más soporte comprimido (`sora`) para
  Evite implementaciones fragmentadas. Las conversiones CAIP-10 aún no se envían.

#### Guía de accesibilidad y uso compartido seguro

- La guía de implementación para superficies de productos se rastrea en vivo
  `docs/portal/docs/reference/address-safety.md`; haga referencia a esa lista de verificación cuando
  adaptando estos requisitos a wallet o explorer UX.
- **Flujos de uso compartido seguro:** Las superficies que copian o muestran direcciones utilizan de forma predeterminada el formulario I105 y exponen una acción de "compartir" adyacente que presenta tanto la cadena completa como un código QR derivado de la misma carga útil para que los usuarios puedan verificar la suma de verificación visualmente o escaneando. Cuando el truncamiento es inevitable (por ejemplo, pantallas pequeñas), conserve el inicio y el final de la cadena, agregue elipses claras y mantenga accesible la dirección completa mediante copia al portapapeles para evitar recortes accidentales.
- **Protección de IME:** Las entradas de dirección DEBEN rechazar los artefactos de composición de los teclados de estilo IME/IME. Aplique la entrada solo ASCII, presente una advertencia en línea cuando se detecten caracteres kana o de ancho completo y ofrezca una zona para pegar texto sin formato que elimine las marcas combinadas antes de la validación para que los usuarios japoneses y chinos puedan desactivar su IME sin perder el progreso.
- **Compatibilidad con lectores de pantalla:** Proporciona etiquetas visualmente ocultas (`aria-label`/`aria-describedby`) que describen los dígitos principales del prefijo Base58 y dividen la carga útil I105 en grupos de 4 u 8 caracteres, de modo que la tecnología de asistencia lea caracteres agrupados en lugar de una cadena continua. Anuncie el éxito de copiar/compartir a través de regiones en vivo educadas y asegúrese de que las vistas previas de QR incluyan texto alternativo descriptivo (“Dirección I105 para <alias> en la cadena 0x02F1”).
- **Uso comprimido solo de Sora:** Etiquete siempre la vista comprimida `i105` como “solo Sora” y ciérrela detrás de una confirmación explícita antes de copiar. Los SDK y las billeteras deben negarse a mostrar resultados comprimidos cuando el discriminante de la cadena no es el valor de Sora Nexus y deben dirigir a los usuarios a I105 para realizar transferencias entre redes para evitar desviar fondos.

## Lista de verificación de implementación

- **Sobre I105:** El prefijo codifica `chain_discriminant` usando el compacto
  Esquema de 6/14 bits de `encode_i105_prefix()`, el cuerpo son los bytes canónicos
  (`AccountAddress::canonical_bytes()`), y la suma de comprobación son los dos primeros bytes
  de Blake2b-512(`b"I105PRE"` || prefijo || cuerpo). La carga útil completa es Base58-
  codificado a través de `bs58`.
- **Contrato de registro:** Publicación JSON firmada (y raíz Merkle opcional)
  `{discriminant, i105_prefix, chain_alias, endpoints}` con TTL de 24 horas y
  teclas de rotación.
- **Política de dominio:** ASCII `Name` hoy; si habilita i18n, aplique UTS-46 para
  normalización y UTS-39 para comprobaciones confusas. Aplicar etiqueta máxima (63) y
  longitudes totales (255).
- **Ayudantes textuales:** Envíe códecs I105 ↔ comprimidos (`i105`) en Rust,
  TypeScript/JavaScript, Python y Kotlin con vectores de prueba compartidos (CAIP-10
  los mapeos siguen siendo trabajo futuro).
- **Herramientas CLI:** Proporciona un flujo de trabajo de operador determinista a través de `iroha tools address convert`
  (ver `crates/iroha_cli/src/address.rs`), que acepta literales I105/`0x…` y
  etiquetas `<address>@<domain>` (rejected legacy form) opcionales, por defecto la salida I105 usa el prefijo Sora Nexus (`753`),
  y solo emite el alfabeto comprimido exclusivo de Sora cuando los operadores lo solicitan explícitamente con
  `--format i105` o el modo de resumen JSON. El comando impone expectativas de prefijo en
  analiza, registra el dominio proporcionado (`input_domain` en JSON) y la bandera `legacy  suffix`
  reproduce la codificación convertida como `<address>@<domain>` (rejected legacy form) para que las diferencias manifiestas sigan siendo ergonómicas.
- **Wallet/explorer UX:** Siga las [directrices para mostrar direcciones] (fuente/sns/address_display_guidelines.md)
  enviado con ADDR-6: ofrece botones de copia dual, mantiene I105 como carga útil QR y advierte
  usuarios que el formulario comprimido `i105` es solo para Sora y susceptible a reescrituras de IME.
- **Integración Torii:** Cache Nexus se manifiesta respetando TTL, emite
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` de manera determinista, y
  keep strict account-literal parsing canonical-I105-only (reject compressed and any `@domain` suffix) with canonical I105 output.

### Formatos de respuesta Torii

- `GET /v1/accounts` acepta un parámetro de consulta opcional `canonical I105 rendering` y
  `POST /v1/accounts/query` acepta el mismo campo dentro del sobre JSON.
  Los valores admitidos son:
  - `i105` (predeterminado): las respuestas emiten cargas útiles I105 Base58 canónicas (p. ej.,
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`).
  - `i105_default` — las respuestas emiten la vista comprimida `i105` exclusiva de Sora mientras
    manteniendo los filtros/parámetros de ruta canónicos.
- Los valores no válidos devuelven `400` (`QueryExecutionFail::Conversion`). Esto permite
  billeteras y exploradores para solicitar cadenas comprimidas para UX solo de Sora mientras
  manteniendo I105 como el predeterminado interoperable.
- Listados de titulares de activos (`GET /v1/assets/{definition_id}/holders`) y su JSON
  La contraparte del sobre (`POST …/holders/query`) también honra a `canonical I105 rendering`.
  El campo `items[*].account_id` emite literales comprimidos siempre que el
  El campo de parámetro/sobre está configurado en `i105_default`, reflejando las cuentas.
  puntos finales para que los exploradores puedan presentar resultados consistentes en todos los directorios.
- **Pruebas:** Agregue pruebas unitarias para viajes de ida y vuelta del codificador/decodificador, cadena incorrecta
  fallas y búsquedas manifiestas; agregar cobertura de integración en Torii y SDK
  para I105 fluye de un extremo a otro.

## Registro de códigos de error

Los codificadores y decodificadores de direcciones exponen fallas a través de
`AccountAddressError::code_str()`. Las siguientes tablas proporcionan los códigos estables.
que los SDK, las billeteras y las superficies Torii deberían aparecer junto con los legibles por humanos
mensajes, además de orientación de solución recomendada.

### Construcción canónica

| Código | Fracaso | Remediación recomendada |
|------|---------|-------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | El codificador recibió un algoritmo de firma que no es compatible con el registro ni con las funciones de compilación. | Restrinja la construcción de cuentas a las curvas habilitadas en el registro y la configuración. |
| `ERR_KEY_PAYLOAD_TOO_LONG` | La longitud de la carga útil de la clave de firma supera el límite admitido. | Los controladores de una sola tecla están limitados a longitudes `u8`; utilice multifirma para claves públicas grandes (por ejemplo, ML‑DSA). |
| `ERR_INVALID_HEADER_VERSION` | La versión del encabezado de dirección está fuera del rango admitido. | Emitir la versión del encabezado `0` para direcciones V1; actualice los codificadores antes de adoptar nuevas versiones. |
| `ERR_INVALID_NORM_VERSION` | No se reconoce el indicador de versión de normalización. | Utilice la versión de normalización `1` y evite alternar bits reservados. |
| `ERR_INVALID_I105_PREFIX` | El prefijo de red I105 solicitado no se puede codificar. | Elija un prefijo dentro del rango `0..=16383` inclusivo publicado en el registro de la cadena. |
| `ERR_CANONICAL_HASH_FAILURE` | Error en el hash de la carga útil canónica. | Vuelva a intentar la operación; Si el error persiste, trátelo como un error interno en la pila de hash. |

### Decodificación de formato y detección automática

| Código | Fracaso | Remediación recomendada |
|------|---------|-------------------------|
| `ERR_INVALID_I105_ENCODING` | La cadena I105 contiene caracteres fuera del alfabeto. | Asegúrese de que la dirección utilice el alfabeto I105 publicado y que no se haya truncado al copiar y pegar. |
| `ERR_INVALID_LENGTH` | La longitud de la carga útil no coincide con el tamaño canónico esperado para el selector/controlador. | Proporcione la carga útil canónica completa para el selector de dominio y el diseño del controlador seleccionados. |
| `ERR_CHECKSUM_MISMATCH` | Error en la validación de la suma de comprobación I105 (preferida) o comprimida (`sora`, segunda mejor). | Regenerar la dirección de una fuente confiable; Esto normalmente indica un error de copiar/pegar. |
| `ERR_INVALID_I105_PREFIX_ENCODING` | Los bytes del prefijo I105 están mal formados. | Vuelva a codificar la dirección con un codificador compatible; no modifique manualmente los bytes principales de Base58. |
| `ERR_INVALID_HEX_ADDRESS` | No se pudo decodificar la forma canónica hexadecimal. | Proporcione una cadena hexadecimal de longitud par y con prefijo `0x` producida por el codificador oficial. |
| `ERR_MISSING_COMPRESSED_SENTINEL` | El formulario comprimido no comienza con `sora`. | Prefije las direcciones Sora comprimidas con el centinela requerido antes de entregarlas a los decodificadores. |
| `ERR_COMPRESSED_TOO_SHORT` | La cadena comprimida carece de dígitos suficientes para la carga útil y la suma de comprobación. | Utilice la cadena comprimida completa emitida por el codificador en lugar de fragmentos truncados. |
| `ERR_INVALID_COMPRESSED_CHAR` | Se encontró un carácter fuera del alfabeto comprimido. | Reemplace el carácter con un glifo Base-105 válido de las tablas publicadas de ancho medio/ancho completo. |
| `ERR_INVALID_COMPRESSED_BASE` | El codificador intentó utilizar una base no compatible. | Presentar un error contra el codificador; el alfabeto comprimido se fija en la base 105 en V1. |
| `ERR_INVALID_COMPRESSED_DIGIT` | El valor del dígito excede el tamaño del alfabeto comprimido. | Asegúrese de que cada dígito esté dentro de `0..105)`, regenerando la dirección si es necesario. |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | La detección automática no pudo reconocer el formato de entrada. | Proporcione cadenas hexadecimales I105 (preferido), comprimidas (`sora`) o canónicas `0x` al invocar analizadores. |

### Validación de dominio y red

| Código | Fracaso | Remediación recomendada |
|------|---------|-------------------------|
| `ERR_DOMAIN_MISMATCH` | El selector de dominio no coincide con el dominio esperado. | Utilice una dirección emitida para el dominio previsto o actualice la expectativa. |
| `ERR_INVALID_DOMAIN_LABEL` | La etiqueta del dominio no superó las comprobaciones de normalización. | Canonicalice el dominio utilizando el procesamiento no transicional UTS-46 antes de la codificación. |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | El prefijo de red I105 decodificado difiere del valor configurado. | Cambie a una dirección de la cadena de destino o ajuste el discriminante/prefijo esperado. |
| `ERR_UNKNOWN_ADDRESS_CLASS` | Los bits de clase de dirección no se reconocen. | Actualice el decodificador a una versión que comprenda la nueva clase o evite alterar los bits del encabezado. |
| `ERR_UNKNOWN_DOMAIN_TAG` | Se desconoce la etiqueta del selector de dominio. | Actualice a una versión que admita el nuevo tipo de selector o evite el uso de cargas útiles experimentales en los nodos V1. |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | Se estableció el bit de extensión reservado. | Borrar bits reservados; permanecen cerrados hasta que una futura ABI los presente. |
| `ERR_UNKNOWN_CONTROLLER_TAG` | Etiqueta de carga útil del controlador no reconocida. | Actualice el decodificador para reconocer nuevos tipos de controladores antes de analizarlos. |
| `ERR_UNEXPECTED_TRAILING_BYTES` | La carga útil canónica contenía bytes finales después de la decodificación. | Regenerar la carga útil canónica; sólo debe estar presente la longitud documentada. |

### Validación de carga útil del controlador

| Código | Fracaso | Remediación recomendada |
|------|---------|-------------------------|
| `ERR_INVALID_PUBLIC_KEY` | Los bytes clave no coinciden con la curva declarada. | Asegúrese de que los bytes clave estén codificados exactamente como se requiere para la curva seleccionada (por ejemplo, Ed25519 de 32 bytes). |
| `ERR_UNKNOWN_CURVE` | El identificador de curva no está registrado. | Utilice el ID de curva `1` (Ed25519) hasta que se aprueben y publiquen curvas adicionales en el registro. |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | El controlador Multisig declara más miembros de los admitidos. | Reduzca la membresía multifirma al límite documentado antes de codificar. |
| `ERR_INVALID_MULTISIG_POLICY` | La carga útil de la política multifirma falló en la validación (umbral/pesos/esquema). | Reconstruya la política para que satisfaga el esquema CTAP2, los límites de peso y las restricciones de umbral. |

## Alternativas consideradas

- **Pure Base58Check (estilo Bitcoin).** Suma de comprobación más simple pero detección de errores más débil
  que la suma de comprobación I105 derivada de Blake2b (`encode_i105` trunca un hash de 512 bits)
  y carece de semántica de prefijo explícita para discriminantes de 16 bits.
- **Incrustar el nombre de la cadena en la cadena del dominio (por ejemplo, `finance@chain`).** Saltos
- **Confíe únicamente en el enrutamiento Nexus sin cambiar direcciones.** Los usuarios aún
  copiar/pegar cadenas ambiguas; queremos que la dirección misma lleve contexto.
- **Sobre Bech32m.** Compatible con QR y ofrece un prefijo legible por humanos, pero
  divergiría de la implementación de envío I105 (`AccountAddress::to_i105`)
  y requieren recrear todos los dispositivos/SDK. La hoja de ruta actual mantiene I105+
  soporte comprimido (`sora`) mientras continúa la investigación sobre el futuro
  Capas Bech32m/QR (el mapeo CAIP-10 está diferido).

## Preguntas abiertas

- Confirmar que los discriminantes `u16` más los rangos reservados cubren la demanda a largo plazo;
  de lo contrario, evalúe `u32` con codificación vaint.
- Finalizar el proceso de gobernanza de firmas múltiples para las actualizaciones de registros y cómo
  Se manejan las revocaciones/asignaciones vencidas.
- Definir el esquema de firma del manifiesto exacto (por ejemplo, Ed25519 multi-sig) y
  Seguridad de transporte (fijación HTTPS, formato hash IPFS) para distribución Nexus.
- Determinar si se admiten alias/redirecciones de dominio para migraciones y cómo
  sacarlos a la luz sin romper el determinismo.
- Especificar cómo los contratos Kotodama/IVM acceden a los ayudantes de I105 (`to_address()`,
  `parse_address()`) y si el almacenamiento en cadena debería exponer alguna vez CAIP-10
  mapeos (hoy I105 es canónico).
- Explorar el registro de cadenas Iroha en registros externos (por ejemplo, registro I105,
  Directorio de espacios de nombres CAIP) para una alineación más amplia del ecosistema.

## Próximos pasos

1. La codificación I105 llegó a `iroha_data_model` (`AccountAddress::to_i105`,
   `parse_encoded`); continuar transfiriendo accesorios/pruebas a cada SDK y purgando cualquier
   Marcadores de posición Bech32m.
2. Amplíe el esquema de configuración con `chain_discriminant` y derive sensato
  valores predeterminados para configuraciones de prueba/desarrollo existentes. **(Hecho: `common.chain_discriminant`
  ahora se envía en `iroha_config`, de forma predeterminada en `0x02F1` con por red
  anula.)**
3. Redactar el esquema de registro Nexus y el editor del manifiesto de prueba de concepto.
4. Recopile comentarios de proveedores y custodios de billeteras sobre aspectos del factor humano.
   (nombramiento HRP, formato de visualización).
5. Actualice la documentación (`docs/source/data_model.md`, documentos de la API de Torii) una vez que
   La ruta de implementación está comprometida.
6. Envíe bibliotecas de códecs oficiales (Rust/TS/Python/Kotlin) con prueba normativa
   vectores que cubren casos de éxito y fracaso.
