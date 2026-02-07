---
lang: es
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T10:35:36.441790+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Vectores de prueba de referencia para trabajos de integración SM2/SM3/SM4.

# Notas de puesta en escena de SM Vectors

Este documento agrega pruebas de respuestas conocidas disponibles públicamente que inicializan los arneses SM2/SM3/SM4 antes de que lleguen los scripts de importación automatizados. Las copias legibles por máquina se encuentran en:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (vectores anexos, casos RFC 8998, Anexo Ejemplo 1).
- `fixtures/sm/sm2_fixture.json` (dispositivo SDK determinista compartido consumido por las pruebas de Rust/Python/JavaScript).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`: corpus curado de 52 casos (accesorios deterministas + bits-flip/mensaje/truncamiento de cola negativos sintetizados) reflejados en Apache-2.0 mientras la suite SM2 ascendente está pendiente. `crates/iroha_crypto/tests/sm2_wycheproof.rs` verifica estos vectores utilizando el verificador SM2 estándar cuando es posible y recurre a una implementación BigInt pura del dominio Anexo cuando es necesario.

## Verificación de firma SM2 contra OpenSSL / Tongsuo / GmSSL

El ejemplo 1 del anexo (Fp-256) utiliza la identidad `ALICE123@YAHOO.COM` (ENTLA 0x0090), el mensaje `"message digest"` y la clave pública que se muestra a continuación. Un flujo de trabajo de copiar y pegar OpenSSL/Tongsuo es:

```bash
# 1. Public key (SubjectPublicKeyInfo)
cat > pubkey.pem <<'PEM'
-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoEcz1UBgi0DQgAECuTHeYqg8RlHG+4RglvkYgK7eeKlhESV6XwE/03y
VIp8AkD4jxzU4WNSpzwXt/FvBzU+U6F21oSp/gxrt5joVw==
-----END PUBLIC KEY-----
PEM

# 2. Message (no trailing newline)
printf "message digest" > msg.bin

# 3. Signature (DER form)
cat > sig.b64 <<'EOF'
MEQCIEDx7Fn3k9n0ngnc70kTDUGU95+x7tLKpVus20nE51XRAiBvxtrDLF1c8Qx337IPfC62Z6RX
hy+wnsVjJ6Z+x97r5w==
EOF
base64 -d sig.b64 > sig.der

# 4. Verify (expects "Signature Verified Successfully")
openssl pkeyutl -verify -pubin -inkey pubkey.pem \
  -in msg.bin -sigfile sig.der \
  -rawin -digest sm3 \
  -pkeyopt distid:ALICE123@YAHOO.COM
```

* OpenSSL 3.x documenta las opciones `distid:`/`hexdistid:`. Algunas compilaciones de OpenSSL 1.1.1 exponen el mando como `sm2_id:`; use el que aparezca en `openssl pkeyutl -help`.
* GmSSL exporta la misma superficie `pkeyutl`; También se aceptaron versiones anteriores `-pkeyopt sm2_id:...`.
* LibreSSL (predeterminado en macOS/OpenBSD) **no** implementa SM2/SM3, por lo que el comando anterior falla allí. Utilice OpenSSL ≥ 1.1.1, Tongsuo o GmSSL.

El asistente DER emite `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7`, que coincide con la firma del anexo.

El anexo también imprime el hash de información del usuario y el resumen resultante:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

Puedes confirmar con OpenSSL:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

Comprobación de cordura de Python para la ecuación de la curva:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## Vectores hash SM3

| Entrada | Codificación hexadecimal | Resumen (hexadecimal) | Fuente |
|-------|--------------|--------------|--------|
| `""` (cadena vacía) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 Anexo A.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 Anexo A.2 |
| `"abcd"` repetido 16 veces (64 bytes) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 Anexo A |

## Vectores de cifrado de bloque SM4 (ECB)

| Llave (hexadecimal) | Texto sin formato (hexadecimal) | Texto cifrado (hexadecimal) | Fuente |
|-----------|-----------------|------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 Anexo A.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 Anexo A.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 Anexo A.3 |

## Cifrado autenticado SM4-GCM

| Clave | IV | DAA | Texto sin formato | Texto cifrado | Etiqueta | Fuente |
|-----|----|-----|-----------|------------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 Apéndice A.2 |

## Cifrado autenticado SM4-CCM| Clave | Nonce | DAA | Texto sin formato | Texto cifrado | Etiqueta | Fuente |
|-----|-------|-----|-----------|------------|-----|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 Apéndice A.3 |

### Casos negativos a prueba de Wyche (SM4 GCM/CCM)

Estos casos informan el conjunto de regresión en `crates/iroha_crypto/tests/sm3_sm4_vectors.rs`. Cada caso debe fallar la verificación.

| Modo | ID de CT | Descripción | Clave | Nonce | DAA | Texto cifrado | Etiqueta | Notas |
|------|-------|-------------|-----|-------|-----|------------|-----|-------|
| GCM | 1 | Voltear el bit de etiqueta | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Etiqueta no válida derivada de Wycheproof |
| MCP | 17 | Voltear el bit de etiqueta | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Etiqueta no válida derivada de Wycheproof |
| MCP | 18 | Etiqueta truncada (3 bytes) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | Garantiza que las etiquetas cortas fallen en la autenticación |
| MCP | 19 | Inversión de bits de texto cifrado | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | Detectar carga útil manipulada |

## Referencia de firma determinista SM2

| Campo | Valor (hexadecimal a menos que se indique lo contrario) | Fuente |
|-------|--------------------------|--------|
| Parámetros de curva | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC`, etc.) | GM/T 0003.5-2012 Anexo A |
| ID de usuario (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 Anexo D |
| Clave pública | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 Anexo D |
| Mensaje | `"message digest"` (hexadecimal `6d65737361676520646967657374`) | GM/T 0003 Anexo D |
| ES | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 Anexo D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 Anexo D |
| Firma `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 Anexo D |
| Multicódec (provisional) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, variante `0x1306`) | Derivado del Anexo Ejemplo 1 |
| Multihash prefijado | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | Derivado (coincide con `sm_known_answers.toml`) |

Las cargas útiles multihash de SM2 están codificadas como `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)`.

### Accesorio de firma determinista del SDK de Rust (SM-3c)

La matriz de vectores estructurados incluye la carga útil de paridad de Rust/Python/JavaScript.
para que cada cliente firme el mismo mensaje SM2 con una semilla compartida y
identificador distintivo.| Campo | Valor (hexadecimal a menos que se indique lo contrario) | Notas |
|-------|--------------------------|-------|
| Identificación distintiva | `"iroha-sdk-sm2-fixture"` | Compartido entre los SDK de Rust/Python/JS |
| Semilla | `"iroha-rust-sdk-sm2-deterministic-fixture"` (hexadecimal `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | Entrada a `Sm2PrivateKey::from_seed` |
| Clave privada | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | Derivado deterministamente de la semilla |
| Clave pública (SEC1 sin comprimir) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Coincide con la derivación determinista |
| Multihash de clave pública | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Salida de `PublicKey::to_string()` |
| Multihash prefijado | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Salida de `PublicKey::to_prefixed_string()` |
| ES | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | Calculado a través de `Sm2PublicKey::compute_z` |
| Mensaje | `"Rust SDK SM2 signing fixture v1"` (hexadecimal `527573742053444B20534D32207369676E696E672066697874757265207631`) | Carga útil canónica para pruebas de paridad del SDK |
| Firma `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | Producido mediante firma determinista |

- Consumo entre SDK:
  - `fixtures/sm/sm2_fixture.json` ahora expone una matriz `vectors`. La suite de regresión criptográfica Rust (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), los asistentes del cliente Rust (`crates/iroha/src/sm.rs`), los enlaces de Python (`python/iroha_python/tests/test_crypto.py`) y el SDK de JavaScript (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) analizan estos dispositivos.
  - `crates/iroha/tests/sm_signing.rs` realiza una firma determinista y verifica que las salidas multihash/multicodec en cadena coincidan con el dispositivo.
  - Los conjuntos de regresión del tiempo de admisión (`crates/iroha_core/tests/admission_batching.rs`) afirman que las cargas útiles de SM2 se rechazan a menos que `allowed_signing` incluya `sm2` *y* `default_hash` sea `sm3-256`, cubriendo las restricciones de configuración de un extremo a otro.
- 追加カバレッジ: 異常系（無効な曲線、異常な `r/s`、`distid` 改ざん）は`crates/iroha_crypto/tests/sm2_fuzz.rs` の propiedad テストで網羅済みです.Anexo Ejemplo 1 の正規ベクトルは `sm_known_answers.toml` に多言語対応の multicodec形式で引き続き提供しています.
- El código Rust ahora expone `Sm2PublicKey::compute_z` para que los dispositivos ZA se puedan generar mediante programación; consulte `sm2_compute_z_matches_annex_example` para ver la regresión del Anexo D.

## Próximas acciones
- Supervisar las regresiones del tiempo de admisión (`admission_batching.rs`) para garantizar que la activación de la configuración continúe aplicando los límites de habilitación de SM2.
- Amplíe la cobertura con casos Wycheproof SM4 GCM/CCM y obtenga objetivos fuzz basados ​​en propiedades para la verificación de firmas SM2. ✅ (Subconjunto de casos no válidos capturado en `sm3_sm4_vectors.rs`).
- LLM solicita ID alternativos: *"Proporcione la firma SM2 para el ejemplo del anexo 1 cuando el ID distintivo esté establecido en 1234567812345678 en lugar de ALICE123@YAHOO.COM, y describa los nuevos valores ZA/e".*