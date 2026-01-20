---
lang: es
direction: ltr
source: docs/account_structure.md
status: complete
translator: manual
source_hash: 7e6a1321c6f8d71ac4b576a55146767fbc488b29c7e21d82bc2e1c55db89769c
source_last_modified: "2025-11-12T00:36:40.117854+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/account_structure.md (Account Structure RFC) -->

# RFC de Estructura de Cuentas

**Estado:** Aceptado (ADDR‑1)  
**Audiencia:** Equipos de modelo de datos, Torii, Nexus, Wallet y Gobernanza  
**Issues relacionadas:** Por definir

## Resumen

Este documento propone una revisión completa del esquema de direccionamiento de
cuentas para proporcionar:

- Una dirección **Iroha Bech32m (IH‑B32)** legible para humanos y con checksum
  que vincula un discriminante de cadena con el signatario de la cuenta y
  ofrece formas textuales deterministas aptas para la interoperabilidad.
- Identificadores de dominio globalmente únicos, respaldados por un registro
  que pueda consultarse a través de Nexus para enrutamiento entre cadenas.
- Capas de transición que mantengan funcionando los alias de enrutamiento
  `alias@domain` mientras migramos wallets, APIs y contratos al nuevo formato.

## Motivación

Hoy en día, los monederos y las herramientas off‑chain dependen de alias de
enrutamiento en bruto `alias@domain`. Eso presenta dos problemas principales:

1. **Sin anclaje de red.** La cadena de texto no incluye ni checksum ni
   prefijo de red, de modo que un usuario puede pegar una dirección de otra
   red sin recibir feedback inmediato. La transacción acabará siendo rechazada
   (desajuste de `chain_id`) o, peor aún, podría aplicarse sobre una
   cuenta diferente si el destino existe localmente.
2. **Colisión de dominios.** Los dominios son solo namespaces y pueden
   reutilizarse en cada cadena. La federación de servicios (custodios, puentes,
   flujos cross‑chain) se vuelve frágil porque `finance` en la cadena A no
   guarda relación con `finance` en la cadena B.

Necesitamos un formato de dirección legible para humanos que proteja frente a
errores de copiar/pegar y un mapeo determinista entre el nombre de dominio y
la cadena autorizada.

## Objetivos

- Diseñar un envoltorio de dirección Bech32m inspirado en IH58 para cuentas de
  Iroha, publicando a la vez alias textuales canónicos CAIP‑10.
- Codificar el discriminante de cadena configurado directamente en cada
  dirección y definir su proceso de gobernanza/registro.
- Describir cómo introducir un registro de dominios global sin romper
  despliegues existentes y especificar reglas de normalización/anti‑spoofing.
- Documentar expectativas operativas, pasos de migración y cuestiones
  abiertas.

## No‑objetivos

- Implementar transferencias de activos cross‑chain. La capa de enrutamiento
  solo devuelve la cadena de destino.
- Cambiar la estructura interna de `AccountId` (sigue siendo
  `DomainId + PublicKey`).
- Finalizar la gobernanza para la emisión de dominios globales. Este RFC se
  centra en el modelo de datos y las primitivas de transporte.

## Contexto

### Alias de enrutamiento actual

```text
AccountId {
    domain: DomainId,   // wrapper sobre Name (string estilo ASCII)
    signatory: PublicKey // string multihash (por ejemplo ed0120...)
}

Display / Parse: "<signatory multihash>@<domain name>"

Esta forma textual se trata ahora como un **alias de cuenta**: una comodidad
de enrutamiento que apunta a la [`AccountAddress`](#2-canonical-address-codecs)
canónica. Sigue siendo útil para la lectura humana y la gobernanza
acotada al dominio, pero ya no se considera el identificador autoritativo de
la cuenta on‑chain.
```

`ChainId` vive fuera de `AccountId`. Los nodos comparan el `ChainId` de la
transacción con la configuración durante la admisión
(`AcceptTransactionFail::ChainIdMismatch`) y rechazan transacciones
foráneas, pero la cadena de texto de la cuenta no transporta ninguna pista
sobre la red.

### Identificadores de dominio

`DomainId` envuelve un `Name` (string normalizada) y está acotado a la cadena
local. Cada cadena puede registrar `wonderland`, `finance`, etc. de forma
independiente.

### Contexto Nexus

Nexus es el responsable de la coordinación cross‑component (lanes/data‑spaces).
Actualmente no tiene noción de enrutamiento de dominios entre cadenas.

## Diseño propuesto

### 1. Discriminante de cadena determinista

Se amplía `iroha_config::parameters::actual::Common` con:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // nuevo, coordinado globalmente
    // ... campos existentes
}
```

- **Restricciones:**
  - Debe ser único por red activa; se gestiona mediante un registro público
    firmado con rangos reservados explícitos (por ejemplo `0x0000–0x0FFF` para
    test/dev, `0x1000–0x7FFF` para asignaciones de comunidad,
    `0x8000–0xFFEF` para espacios aprobados por gobernanza,
    `0xFFF0–0xFFFF` reservado).
  - Inmutable para una cadena en ejecución. Cambiarlo requiere hard fork y
    actualización del registro.
- **Gobernanza y registro:** un conjunto de gobernanza multi‑firma mantiene un
  registro JSON firmado (y fijado en IPFS) que mapea discriminantes a alias
  humanos e identificadores CAIP‑2. Los clientes obtienen y cachean este
  registro para validar y mostrar metadatos de cadena.
- **Uso:** el valor se inyecta a través de la admisión de estado, Torii,
  SDKs y APIs de wallet, de forma que cada componente pueda incluirlo o
  validarlo. Se expone como parte de CAIP‑2 (por ejemplo `iroha:0x1234`).

<a id="2-canonical-address-codecs"></a>

### 2. Codecs de dirección canónica

El modelo de datos en Rust expone una única representación binaria canónica
(`AccountAddress`) que puede emitirse en varios formatos de cara al usuario:

- **IH58 (Iroha Base58)**: un envoltorio Base58 que incluye el discriminante
  de cadena. Los decoders validan el prefijo antes de promover la payload a la
  forma canónica.
- **Vista comprimida de Sora:** alfabeto exclusivo de Sora con **105 símbolos**
  construido añadiendo el poema イロハ en half‑width (incluyendo ヰ y ヱ) al
  conjunto IH58 de 58 caracteres. Las cadenas empiezan con el sentinel `snx1`,
  incluyen un checksum derivado de Bech32m y omiten el prefijo de red
  (Sora Nexus se da por implícito en el sentinel).

  ```text
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Hex canónico:** representación `0x…` amigable para depuración de la
  envoltura binaria canónica.

`AccountAddress::parse_any` auto‑detecta entradas comprimidas, IH58 o hex
canónico y devuelve tanto la payload decodificada como el
`AccountAddressFormat` detectado. Torii utiliza `parse_any` para direcciones
suplementarias ISO 20022 y almacena la forma hex canónica, de modo que los
metadatos permanezcan deterministas con independencia de la representación
original.

#### 2.1 Layout del byte de cabecera (ADDR‑1a)

Cada payload canónica se organiza como `header · domain selector · controller`.
La `header` es un único byte que indica qué reglas de parseo se aplican a los
bytes siguientes:

```text
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

Ese primer byte empaqueta la metadata de esquema que necesitan los decoders:

| Bits | Campo          | Valores permitidos | Error si se viola                                    |
|------|----------------|--------------------|------------------------------------------------------|
| 7‑5  | `addr_version` | `0` (v1). Valores `1‑7` reservados para futuras revisiones. | Valores fuera de `0‑7` disparan `AccountAddressError::InvalidHeaderVersion`; las implementaciones DEBEN tratar valores distintos de cero como no soportados hoy. |
| 4‑3  | `addr_class`   | `0` = clave única, `1` = multisig. | Otros valores producen `AccountAddressError::UnknownAddressClass`. |
| 2‑1  | `norm_version` | `1` (Norm v1). Valores `0`, `2`, `3` reservados. | Valores fuera de `0‑3` producen `AccountAddressError::InvalidNormVersion`. |
| 0    | `ext_flag`     | DEBE ser `0`.      | Bit activado → `AccountAddressError::UnexpectedExtensionFlag`. |

El encoder en Rust escribe siempre `0x02` (versión 0, clase de clave única,
versión de normalización 1, bit de extensión a cero).

#### 2.2 Codificación del selector de dominio (ADDR‑1a)

El selector de dominio sigue inmediatamente a la cabecera y es una unión
etiquetada:

| Tag   | Significado              | Payload   | Notas |
|-------|--------------------------|-----------|-------|
| `0x00` | Dominio por defecto implícito | ninguno   | Coincide con el `default_domain_name()` configurado. |
| `0x01` | Digest de dominio local | 12 bytes | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Entrada de registro global | 4 bytes  | `registry_id` en big‑endian; reservado hasta que exista el registro global. |

Las etiquetas de dominio se canonizan (UTS‑46 + STD3 + NFC) antes de
hashearlas. Tags desconocidos producen `AccountAddressError::UnknownDomainTag`.
Cuando se valida una dirección contra un dominio, un selector que no coincide
resulta en `AccountAddressError::DomainMismatch`.

```text
domain selector
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

El selector está contiguo al payload del controlador, de modo que el decoder
puede recorrer el formato on‑wire en orden: leer el byte de tag, leer la
payload específica de ese tag y avanzar al bloque de controlador.

**Ejemplos de selector**

- *Por defecto implícito* (`tag = 0x00`). Sin payload. Ejemplo de hex canónico
  para el dominio por defecto con la clave de prueba determinista:
  `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.
- *Digest local* (`tag = 0x01`). La payload es el digest de 12 bytes. Ejemplo
  (`treasury`, seed `0x01`):
  `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.
- *Registro global* (`tag = 0x02`). La payload es un `registry_id:u32`
  big‑endian. Los bytes que siguen a la payload son idénticos al caso de
  dominio por defecto implícito; el selector simplemente sustituye la cadena
  de dominio normalizada por un puntero de registro. Ejemplo con
  `registry_id = 0x0000_002A` (42 en decimal) y el controlador deterministic
  por defecto:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.
  Desglose: cabecera `0x02`, tag de selector `0x02`, id de registro
  `00 00 00 2A`, tag de controlador `0x00`, id de curva `0x01`, longitud
  `0x20` y payload de clave Ed25519 de 32 bytes.

#### 2.3 Codificación del payload de controlador (ADDR‑1a)

El payload del controlador es otra unión etiquetada que se añade tras el
selector de dominio:

| Tag   | Controlador | Layout | Notas |
|-------|-------------|--------|-------|
| `0x00` | Clave única | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id = 0x01` corresponde hoy a Ed25519. `key_len` está acotado a `u8`; valores mayores producen `AccountAddressError::KeyPayloadTooLong`. |
| `0x01` | Multisig    | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | Suporta até 255 membros (`CONTROLLER_MULTISIG_MEMBER_MAX`). Curvas desconhecidas produzem `AccountAddressError::UnknownCurve`; políticas malformadas sobem como `AccountAddressError::InvalidMultisigPolicy`. |

Las políticas multisig exponen además un mapa CBOR al estilo CTAP2 y un
digest canónico, de modo que hosts y SDKs puedan verificar el controlador de
forma determinista. Véase
`docs/source/references/address_norm_v1.md` (ADDR‑1c) para el esquema, el
procedimiento de hashing y los fixtures de referencia.

Todos los bytes de claves se codifican exactamente como los devuelve
`PublicKey::to_bytes`; los decoders reconstruyen instancias `PublicKey` y
emiten `AccountAddressError::InvalidPublicKey` si los bytes no coinciden con
la curva declarada.

> **Aplicación canónica Ed25519 (ADDR‑3a):** las claves con curva `0x01`
> deben decodificar exactamente a la cadena de bytes emitida por el firmante y
> no pueden caer en el subgrupo de orden pequeño. Los nodos rechazan
> codificaciones no canónicas (por ejemplo, valores reducidos módulo
> `2^255‑19`) y puntos débiles como el elemento identidad, por lo que los SDKs
> deberían exponer errores de validación equivalentes antes de enviar
> direcciones.

##### 2.3.1 Registro de identificadores de curva (ADDR‑1d)

| ID (`curve_id`) | Algoritmo | Feature gate | Notas |
|-----------------|-----------|--------------|-------|
| `0x00` | Reservado | — | NO DEBE emitirse; los decoders devuelven `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519   | Siempre activo | Algoritmo canónico v1 (`Algorithm::Ed25519`); único id habilitado hoy en builds de producción. |
| `0x02` | ML‑DSA (preview) | `ml-dsa` | Reservado para el despliegue de ADDR‑3; desactivado por defecto hasta que exista la ruta de firma ML‑DSA. |
| `0x03` | Marcador GOST | `gost` | Reservado para aprovisionamiento/negociación; los payloads con este id deben rechazarse hasta que gobernanza apruebe un perfil TC26 concreto. |
| `0x0A` | GOST R 34.10‑2012 (256, conjunto A) | `gost` | Reservado; se habilitará junto al feature criptográfico `gost`. |
| `0x0B` | GOST R 34.10‑2012 (256, conjunto B) | `gost` | Reservado para futura aprobación de gobernanza. |
| `0x0C` | GOST R 34.10‑2012 (256, conjunto C) | `gost` | Reservado para futura aprobación de gobernanza. |
| `0x0D` | GOST R 34.10‑2012 (512, conjunto A) | `gost` | Reservado para futura aprobación de gobernanza. |
| `0x0E` | GOST R 34.10‑2012 (512, conjunto B) | `gost` | Reservado para futura aprobación de gobernança. |
| `0x0F` | SM2       | `sm`   | Reservado; ficará disponível quando o feature criptográfico SM estiver estável. |

Os slots `0x04–0x09` permanecen livres para curvas adicionais; introduzir um
novo algoritmo exige atualização de roadmap e suporte correspondente em
SDK/host. Encoders DEVEM rejeitar algoritmos não suportados com
`ERR_UNSUPPORTED_ALGORITHM`, e decoders DEVEM falhar rapidamente em ids
desconhecidos com `ERR_UNKNOWN_CURVE` para manter comportamento fail‑closed.

El registro canónico (incluindo um export JSON legível por máquinas) vive em
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md).
Ferramentas DEVEM consumi‑lo diretamente para que os identificadores de curva
permaneçam consistentes entre SDKs e fluxos operacionais.

- **Gating em SDKs:** SDKs vêm por padrão apenas com Ed25519. Swift expone
  flags de compilação (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`), o SDK Java exige chamada a
  `AccountAddress.configureCurveSupport(...)`, e o SDK JavaScript usa
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`,
  de modo que integradores precisan fazer opt‑in explícito antes de emitir
  endereços com curvas adicionais.
- **Gating no host:** `Register<Account>` rejeita controladores cujos
  signatários usam algoritmos ausentes de `crypto.allowed_signing` **ou**
  identificadores de curva ausentes de `crypto.curves.allowed_curve_ids`, de
  forma que clusters precisan anunciar suporte (configuração + gênese) antes
  de permitir controladores ML‑DSA/GOST/SM.

#### 2.4 Regras de falha (ADDR‑1a)

- Payloads menores que o header + selector obrigatórios, ou com bytes
  sobrando, emitem `AccountAddressError::InvalidLength` ou
  `AccountAddressError::UnexpectedTrailingBytes`.
- Headers que ativam o `ext_flag` reservado ou anunciam versões/classes
  não suportadas DEVEM ser rejeitados com `UnexpectedExtensionFlag`,
  `InvalidHeaderVersion` ou `UnknownAddressClass`.
- Tags desconhecidas de selector/controlador resultam em
  `UnknownDomainTag` ou `UnknownControllerTag`.
- Material de chave grande demais ou malformado gera
  `KeyPayloadTooLong` ou `InvalidPublicKey`.
- Controladores multisig com mais de 255 membros geram
  `MultisigMemberOverflow`.
- Conversões IME/NFKC: kana Sora em half‑width podem ser normalizadas para
  full‑width sem quebrar o decode, mas o sentinel ASCII `snx1` e os dígitos/
  letras IH58 DEVEM permanecer ASCII. Sentinels em full‑width ou com
  case‑folding provocam `ERR_MISSING_COMPRESSED_SENTINEL`, payloads ASCII em
  full‑width geram `ERR_INVALID_COMPRESSED_CHAR` e erros de checksum sobem
  como `ERR_CHECKSUM_MISMATCH`. Property tests em
  `crates/iroha_data_model/src/account/address.rs` cobrem estes caminhos para
  que SDKs e wallets possam contar com falhas determinísticas.
- O parse de aliases `address@domain` em Torii e nos SDKs agora retorna os
  mesmos códigos `ERR_*` quando entradas IH58/comprimidas falham antes do
  fallback ao alias (por exemplo, checksum inválido ou digest de domínio em
  conflito), permitindo que os clientes repassem motivos estruturados em vez
  de inferir mensagens textuais.

#### 2.5 Vectores binarios normativos

- **Dominio por defecto implícito (`default`, byte seed `0x00`)**  
  Hex canónico:
  `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.  
  Desglose: cabecera `0x02`, selector `0x00` (default implícito), tag de
  controlador `0x00`, id de curva `0x01` (Ed25519), longitud `0x20` y, a
  continuación, los 32 bytes de la clave.
- **Digest de dominio local (`treasury`, byte seed `0x01`)**  
  Hex canónico:
  `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.  
  Desglose: cabecera `0x02`, tag de selector `0x01` más el digest
  `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, seguido del payload de clave única
  (`0x00` como tag de controlador, `0x01` como id de curva, `0x20` como
  longitud y los 32 bytes de la clave Ed25519).

Los tests de unidad (`account::address::tests::parse_any_accepts_all_formats`)
comprueban estos vectores V1 mediante `AccountAddress::parse_any`, garantizando
que las herramientas puedan confiar en el mismo payload canónico a través de
representaciones hex, IH58 y comprimidas. El conjunto extendido de fixtures se
puede regenerar con:

```text
cargo xtask address-vectors --out fixtures/account/address_vectors.json
```

### 3. Dominios globalmente únicos y normalización

Consulte también
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
para ver la canalización Norm v1 canónica utilizada en Torii, el modelo de
datos y los SDKs.

Se redefine `DomainId` como tupla etiquetada:

```text
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // nuevo enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // por defecto para la cadena local
    External { chain_discriminant: u16 },
}
```

`LocalChain` envuelve el `Name` existente para dominios gestionados por la
cadena actual. Cuando un dominio se registra a través del registro global, se
persiste el discriminante de la cadena propietaria. La forma de mostrar/parsear
permanece igual por ahora, pero la estructura ampliada permite decisiones de
enrutamiento.

#### 3.1 Normalización y defensas contra suplantación

Norm v1 define la canalización canónica que todos los componentes deben usar
antes de persistir un nombre de dominio o de incrustarlo en un
`AccountAddress`. El recorrido completo está en
`docs/source/references/address_norm_v1.md`; el resumen siguiente recoge los
pasos que deben implementar wallets, Torii, SDKs y herramientas de gobernanza.

1. **Validación de entrada.** Rechazar cadenas vacías, espacios en blanco y
   los delimitadores reservados `@`, `#`, `$`. Esto coincide con las
   invariantes de `Name::validate_str`.
2. **Composición Unicode NFC.** Aplicar normalización NFC (respaldada por ICU)
   para colapsar de forma determinista secuencias canónicamente equivalentes
   (por ejemplo `e\u{0301}` → `é`).
3. **Normalización UTS‑46.** Ejecutar la salida NFC a través de UTS‑46 con
   `use_std3_ascii_rules = true`, `transitional_processing = false` y límites
   de longitud DNS activados. El resultado es una secuencia de A‑labels en
   minúsculas; entradas que violen STD3 fallan aquí.
4. **Límites de longitud.** Aplicar los límites estilo DNS: cada label debe
   tener entre 1 y 63 bytes y el dominio completo NO debe superar 255 bytes
   tras el paso 3.
5. **Política opcional de confundibles.** Las comprobaciones de script
   UTS‑39 se rastrean para Norm v2; los operadores pueden activarlas antes,
   pero un fallo en esta comprobación debe abortar el procesamiento.

Si todos los pasos tienen éxito, la cadena de A‑labels en minúsculas se
cachea y se utiliza para codificación de direcciones, configuración,
manifiestos y consultas a registros. Los selectores de digest local derivan su
valor de 12 bytes como
`blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]` usando la
salida del paso 3. Cualquier otra entrada (mezcla de mayúsculas/minúsculas,
Unicode crudo, etc.) se rechaza con `ParseError`s estructurados en el punto
donde se suministró el nombre.

Fixtures canónicos que ilustran estas reglas —incluidos round‑trips punycode
y secuencias STD3 inválidas— se describen en
`docs/source/references/address_norm_v1.md` y se reflejan en los vectores de
tests de CI de SDKs rastreados bajo ADDR‑2.

### 4. Registro de dominios en Nexus y enrutamiento

- **Esquema de registro:** Nexus mantiene un mapa firmado
  `DomainName -> ChainRecord`, donde `ChainRecord` incluye el discriminante de
  cadena, metadatos opcionales (endpoints RPC) y una prueba de autoridad
  (por ejemplo, multi‑firma de gobernanza).
- **Mecanismo de sincronización:**
  - Las cadenas envían a Nexus reclamaciones de dominio firmadas (en génesis
    o mediante instrucciones de gobernanza).
  - Nexus publica manifiestos periódicos (JSON firmado más raíz Merkle
    opcional) sobre HTTPS y almacenamiento direccionado por contenido
    (p. ej. IPFS). Los clientes fijan (`pin`) el manifiesto más reciente y
    verifican sus firmas.
- **Flujo de resolución:**
  - Torii recibe una transacción que hace referencia a un `DomainId`.
  - Si el dominio es desconocido localmente, Torii consulta el manifiesto
    cacheado de Nexus.
  - Si el manifiesto indica que el dominio pertenece a una cadena externa, la
    transacción se rechaza con un error determinista `ForeignDomain` y la
    información de la cadena remota.
  - Si el dominio no figura en Nexus, Torii devuelve `UnknownDomain`.
- **Anclas de confianza y rotación:** claves de gobernanza firman los
  manifiestos; la rotación o revocación se publica como una nueva versión del
  manifiesto. Los clientes aplican TTLs de manifiesto (por ejemplo 24 h) y
  se niegan a consultar datos obsoletos más allá de esa ventana.
- **Modos de fallo:** si la obtención del manifiesto falla, Torii recurre al
  manifiesto cacheado mientras su TTL siga vigente; pasado el TTL emite
  `RegistryUnavailable` y rechaza el enrutamiento entre dominios para evitar
  estados inconsistentes.

#### 4.1 Inmutabilidad del registro, alias y tombstones (ADDR‑7c)

Nexus publica un **manifiesto append‑only**, de modo que cada asignación de
dominio o alias pueda auditarse y reproducirse. Los operadores deben tratar el
bundle descrito en
`docs/source/runbooks/address_manifest_ops.md` como única fuente de verdad:
si falta un manifiesto o no pasa la validación, Torii debe negarse a resolver
el dominio afectado.

Soporte de automatización:
`cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
reproduce las comprobaciones de checksum, esquema y `previous_digest` que
figuran en el runbook. Incluya la salida del comando en los tickets de cambio
para demostrar que se verificó el enlace `sequence` + `previous_digest` antes
de publicar el bundle.

##### Cabecera de manifiesto y contrato de firma

| Campo            | Requisito |
|------------------|-----------|
| `version`        | Actualmente `1`. Solo debe incrementarse junto con una actualización del spec. |
| `sequence`       | Se incrementa en **exactamente** uno por publicación. Torii rechaza revisiones con saltos o regresiones. |
| `generated_ms` + `ttl_hours` | Definen la frescura de caché (por defecto 24 h). Si el TTL expira antes del siguiente manifiesto, Torii pasa a `RegistryUnavailable`. |
| `previous_digest` | Digest BLAKE3 (hex) del cuerpo del manifiesto previo. Los verificadores lo recalculan con `b3sum` para garantizar inmutabilidad. |
| `signatures`     | Los manifiestos se firman vía Sigstore (`cosign sign-blob`). Operaciones debe ejecutar `cosign verify-blob --bundle manifest.sigstore manifest.json` y comprobar identidad/emisor de gobernanza antes del despliegue. |

La automatización de releases genera `manifest.sigstore` y `checksums.sha256`
junto al cuerpo JSON. Deben mantenerse juntos al replicar en SoraFS o
endpoints HTTP, de modo que los auditores puedan reproducir los pasos de
verificación de forma literal.

##### Tipos de entrada

| Tipo           | Propósito | Campos requeridos |
|----------------|-----------|-------------------|
| `global_domain` | Declara que un dominio está registrado globalmente y debe mapearse a un discriminante de cadena y un prefijo IH58. | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `local_alias`   | Rastrea selectores heredados (`Local-12`) que siguen enroutando localmente. Añade el digest de 12 bytes y un `alias_label` opcional. | `{ "domain": "<label>", "selector": { "kind": "local", "digest_hex": "<12-byte-hex>" }, "alias_label": "<optional>" }` |
| `tombstone`     | Retira un alias/selector de forma permanente. Obligatorio al borrar digests Local‑8 o eliminar un dominio. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

Las entradas `global_domain` pueden incluir opcionalmente un `manifest_url` o
`sorafs_cid` para apuntar a metadatos de cadena firmados, pero la tupla
canónica sigue siendo `{domain, chain, discriminant/ih58_prefix}`.
Los registros `tombstone` **deben** indicar qué selector se retira y el
ticket/artefacto de gobernanza que autorizó el cambio, de forma que pueda
reconstruirse la traza de auditoría offline.

##### Flujo de alias/tombstone y telemetría

1. **Detectar deriva.** Use
   `torii_address_local8_total{endpoint}` y
   `torii_address_invalid_total{endpoint,reason}`
   (representados en `dashboards/grafana/address_ingest.json`) para confirmar
   que las cadenas Local‑8 ya no se aceptan en producción antes de proponer un
   tombstone.
2. **Derivar digests canónicos.** Ejecute
   `iroha address convert <address-or-account_id> --format json --expect-prefix 753`
   (o consuma `fixtures/account/address_vectors.json` vía
   `scripts/account_fixture_helper.py`) para capturar exactamente el
   `digest_hex`. El CLI acepta entradas como `snx1...@wonderland`; el resumen
   JSON expone el dominio vía `input_domain` y `--append-domain` reproduce la
   codificación convertida como `<ih58>@wonderland` para actualizar el
   manifiesto. Para exportaciones orientadas a líneas, use
   para convertir masivamente selectores Local en formas IH58 canónicas (o
   comprimidas/hex/JSON), omitiendo filas no locales. Para evidencia apta para
   hojas de cálculo, ejecute
   y genere un CSV (`input,status,format,domain_kind,…`) que destaque
   selectores Local, codificaciones canónicas y fallos de parse en un único
   archivo.
3. **Añadir entradas al manifiesto.** Redacte el registro `tombstone` (y el
   `global_domain` posterior cuando se migre al registro global) y valide el
   manifiesto con `cargo xtask address-manifest verify` antes de solicitar
   firmas.
4. **Verificar y publicar.** Siga la checklist del runbook (hashes, Sigstore,
   monotonía de `sequence`) antes de replicar el bundle en SoraFS. Torii usa
   de producción aplican inmediatamente literales IH58/comprimidos canónicos
   tras aterrizar el bundle.
5. **Monitorizar y, si es necesario, revertir.** Mantenga los paneles Local‑8
   en cero durante 30 días; si aparecen regresiones, vuelva a publicar el
   bundle de manifiesto previo y, solo en entornos no productivos, establezca
   telemetría.
