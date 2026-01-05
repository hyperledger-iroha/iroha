---
lang: es
direction: ltr
source: docs/genesis.md
status: complete
translator: manual
source_hash: 975c403019da0b8700489610d86a75f26886f40f2b4c10963ee8269a68b4fe9b
source_last_modified: "2025-11-12T00:33:55.324259+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/genesis.md (Genesis configuration) -->

# Configuración de Génesis

Un archivo `genesis.json` define las primeras transacciones que se ejecutan
cuando se inicia una red Iroha. El archivo es un objeto JSON con estos campos:

- `chain`: identificador de cadena único.
- `executor` (opcional): ruta al bytecode del executor (`.to`). Si está
  presente, génesis incluye una instrucción `Upgrade` como primera transacción.
  Si se omite, no se realiza ninguna actualización y se usa el executor
  incorporado.
- `ivm_dir`: directorio que contiene las librerías de bytecode de IVM. Si se
  omite, el valor por defecto es `"."`.
- `consensus_mode`: modo de consenso anunciado en el manifest. Requerido; usa `"Npos"` para Iroha3 (por defecto) y `"Permissioned"` para Iroha2.
- `transactions`: lista de transacciones de génesis que se ejecutan
  secuencialmente. Cada entrada puede contener:
  - `parameters`: parámetros iniciales de la red.
  - `instructions`: instrucciones codificadas con Norito.
  - `ivm_triggers`: triggers con ejecutables de bytecode de IVM.
  - `topology`: topología inicial de peers. Cada entrada usa `peer` (PeerId
    como string, es decir, la clave pública) y `pop_hex`; `pop_hex` puede
    omitirse al componer, pero debe estar presente antes de firmar.
- `crypto`: snapshot criptográfico que refleja `iroha_config.crypto`
  (`default_hash`, `allowed_signing`, `allowed_curve_ids`,
  `sm2_distid_default`, `sm_openssl_preview`). `allowed_curve_ids` refleja
  `crypto.curves.allowed_curve_ids` para que los manifests puedan anunciar qué
  curvas de controladores acepta el clúster. Las herramientas aplican
  combinaciones SM válidas: manifests que declaran `sm2` también deben cambiar
  el hash a `sm3-256`, mientras que las builds compiladas sin el feature `sm`
  rechazan `sm2` por completo.

Ejemplo (salida de `kagami genesis generate default --consensus-mode npos`, con instrucciones
recortadas):

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [78, 82, 84, 48, 0, 0, 19, 123, ...],
      "ivm_triggers": [],
      "topology": []
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### Inicializar el bloque `crypto` para SM2/SM3

Utiliza el helper `xtask` para producir en un solo paso el inventario de
claves y el snippet de configuración listo para pegar:

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

Ahora `client-sm2.toml` contiene:

```toml
# Account key material
public_key = "sm2:8626530010..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]  # eliminar "sm2" para permanecer en modo sólo-verificación
allowed_curve_ids = [1]               # añadir nuevos curve ids (p. ej., 15 para SM2) cuando se permitan controladores
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # opcional: sólo cuando se despliegue la ruta OpenSSL/Tongsuo
```

Copia los valores de `public_key`/`private_key` en la configuración de la
cuenta/cliente y actualiza el bloque `crypto` de `genesis.json` para que
coincida con el snippet (por ejemplo, establece `default_hash` en `sm3-256`,
añade `"sm2"` a `allowed_signing` e incluye los `allowed_curve_ids`
correctos). Kagami rechazará manifests donde los ajustes de hash/curvas y la
lista de firmas permitidas sean inconsistentes.

> **Consejo:** Envía el snippet a stdout con `--snippet-out -` cuando sólo
> quieras inspeccionar el resultado. Usa `--json-out -` para emitir también el
> inventario de claves por stdout.

Si prefieres invocar manualmente los comandos de bajo nivel del CLI, el flujo
equivalente es:

```bash
# 1. Producir material de clave determinista (escribe JSON en disco)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. Regenerar el snippet que se puede pegar en archivos de cliente/config
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **Consejo:** `jq` se usa arriba para evitar un paso de copia/pega manual. Si
> no está disponible, abre `sm2-key.json`, copia el campo `private_key_hex` y
> pásalo directamente a `crypto sm2 export`.

> **Guía de migración:** al convertir una red existente a SM2/SM3/SM4, sigue
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> para aplicar los overrides en capas de `iroha_config`, regenerar manifests y
> planificar el rollback.

## Generar y validar

1. Generar una plantilla:

   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     [--genesis-public-key <public-key>] \
     > genesis.json
   ```

   - `--executor` (opcional) apunta a un archivo `.to` de executor IVM; si está
     presente, Kagami emite una instrucción `Upgrade` como primera transacción.
   - `--genesis-public-key` establece la clave pública que se usará para firmar
     el bloque de génesis; debe ser un multihash reconocido por
     `iroha_crypto::Algorithm` (incluidos los GOST TC26 cuando se compila con
     el feature correspondiente).
   - En Iroha3, `--consensus-mode npos` es obligatorio y no se admiten cutovers escalonados; en Iroha2 el modo por defecto es `permissioned`.

2. Validar mientras editas:

   ```bash
   cargo run -p iroha_kagami -- genesis validate genesis.json
   ```

   Esto verifica que `genesis.json` respete el esquema, que los parámetros sean
   válidos, que los nombres (`Name`) sean correctos y que las instrucciones
   Norito se puedan decodificar.

3. Firmar para despliegue:

   ```bash
   cargo run -p iroha_kagami -- genesis sign \
     genesis.json \
     --public-key <PK> \
     --private-key <SK> \
     --out-file genesis.signed.nrt
   ```

   Cuando se inicia `irohad` sólo con `--genesis-manifest-json` (sin bloque de
   génesis firmado), el nodo inicializa automáticamente su configuración
   criptográfica en tiempo de ejecución a partir del manifest; si además se
   proporciona un bloque de génesis, el manifest y la configuración deben
   coincidir exactamente.

`kagami genesis sign` comprueba que el JSON sea válido y produce un bloque
codificado con Norito listo para usarse mediante `genesis.file` en la
configuración del nodo. El archivo resultante `genesis.signed.nrt` ya está en
forma de wire canónica: un byte de versión seguido de una cabecera Norito que
describe el layout de la payload. Distribuye siempre esta salida enmarcada.
Se recomienda el sufijo `.nrt` para payloads firmados; si no necesitas
actualizar el executor en génesis, puedes omitir el campo `executor` y no
proporcionar ningún archivo `.to`.

Al firmar manifiestos NPoS (`--consensus-mode npos` o con un cutover escalonado solo en Iroha2), `kagami genesis sign` requiere el payload `sumeragi_npos_parameters`; genérelo con `kagami genesis generate --consensus-mode npos` o añade el parámetro manualmente.
Por defecto, `kagami genesis sign` usa el `consensus_mode` del manifest; pasa `--consensus-mode` para anularlo.

## Qué puede hacer Génesis

Génesis admite las siguientes operaciones. Kagami las compone en transacciones
en un orden bien definido para que los peers ejecuten de forma determinista la
misma secuencia.

- **Parámetros**: establecer valores iniciales para Sumeragi (tiempos de
  bloque/commit, drift), Block (máximo de transacciones), Transaction
  (máximo de instrucciones, tamaño de bytecode), límites del executor y de
  contratos inteligentes (combustible, memoria, profundidad) y parámetros
  personalizados. Kagami siembra `Sumeragi::NextMode` y la payload
  `sumeragi_npos_parameters` en el bloque `parameters`, y el bloque firmado
  incluye las instrucciones `SetParameter` generadas para que el arranque
  pueda aplicar ajustes de consenso desde estado on‑chain.
- **Instrucciones nativas**: registrar/anular registro de dominios, cuentas y
  definiciones de activos; acuñar/quemar/transferir activos; transferir
  propiedad de dominios y definiciones de activos; modificar metadatos; otorgar
  permisos y roles.
- **Triggers de IVM**: registrar triggers que ejecutan bytecode de IVM (ver
  `ivm_triggers`). Los ejecutables de los triggers se resuelven de forma
  relativa a `ivm_dir`.
- **Topología**: proporcionar el conjunto inicial de peers mediante el array
  `topology` dentro de cualquier transacción (habitualmente la primera o la última).
  Cada entrada es `{ "peer": "<public_key>", "pop_hex": "<hex>" }`; `pop_hex` puede
  omitirse al componer, pero debe estar presente antes de firmar.
- **Actualización del executor (opcional)**: si `executor` está presente,
  génesis inserta una única instrucción `Upgrade` como primera transacción; si
  no, génesis empieza directamente con parámetros/instrucciones.

### Orden de las transacciones

Conceptualmente, las transacciones de génesis se procesan en este orden:

1. (Opcional) Upgrade del executor  
2. Para cada transacción en `transactions`:
   - Actualizaciones de parámetros
   - Instrucciones nativas
   - Registro de triggers IVM
   - Entradas de topología

Kagami y el código del nodo garantizan este orden para que, por ejemplo, los
parámetros se apliquen antes de las instrucciones posteriores dentro de la
misma transacción.

## Flujo de trabajo recomendado

- Partir de una plantilla generada con Kagami:
  - Sólo ISI integrados:  
    `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json` (Iroha3 por defecto; usa `--consensus-mode permissioned` para Iroha2)
  - Con actualización opcional de executor: añadir `--executor <path/to/executor.to>`
- `<PK>` es cualquier multihash reconocido por `iroha_crypto::Algorithm`,
  incluidos los variantes GOST TC26 cuando Kagami se compila con
  `--features gost` (por ejemplo `gost3410-2012-256-paramset-a:...`).
- Validar mientras se edita: `kagami genesis validate genesis.json`
- Firmar para despliegue:
  `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- Configurar los peers: establecer `genesis.file` apuntando al archivo Norito
  firmado (por ejemplo `genesis.signed.nrt`) y `genesis.public_key` al mismo
  `<PK>` usado para firmar.

Notas:
- La plantilla “default” de Kagami registra un dominio y cuentas de ejemplo,
  acuña algunos activos y concede permisos mínimos utilizando sólo ISIs
  integradas, sin necesidad de `.to`.
- Si incluyes una actualización de executor, debe ser la primera transacción.
  Kagami lo aplica al generar/firmar.
- Usa `kagami genesis validate` para detectar valores `Name` inválidos
  (p. ej., espacios en blanco) e instrucciones mal formadas antes de firmar.

## Ejecución con Docker/Swarm

Las configuraciones de Docker Compose y Swarm proporcionadas manejan ambos
casos:

- Sin executor: el comando `compose` elimina un campo `executor` ausente/vacío
  y firma el archivo.
- Con executor: resuelve la ruta relativa del executor a una ruta absoluta
  dentro del contenedor y firma el archivo.

Esto mantiene simple el desarrollo en máquinas sin muestras de IVM precompiladas
y, al mismo tiempo, permite actualizaciones de executor cuando sea necesario.
