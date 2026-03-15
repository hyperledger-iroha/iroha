---
lang: es
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones-de-servicio-de-rompecabezas
título: Guía de operaciones del servicio Puzzle
sidebar_label: Operaciones de servicio de rompecabezas
descripción: Entradas de admisión a Argon2/ML-DSA کے لئے `soranet-puzzle-service` daemon کی آپریشنز۔
---

:::nota Fuente canónica
`docs/source/soranet/puzzle_service_operations.md` کی عکاسی کرتا ہے۔ جب تک پرانا conjunto de documentación retirar نہ ہو، دونوں ورژنز sincronización رکھیں۔
:::

# Guía de operaciones del servicio de rompecabezas

`tools/soranet-puzzle-service/` y demonio `soranet-puzzle-service`
Boletos de admisión respaldados por Argon2 جاری کرتا ہے جو relé کی Política `pow.puzzle.*`
کو espejo کرتے ہیں، اور جب configurar ہو تو relés de borde کی جانب سے ML-DSA
corredor de tokens de admisión کرتا ہے۔ Varios puntos finales HTTP exponen کرتا ہے:

- `GET /healthz` - sonda de vida.
- `GET /v2/puzzle/config` - relé JSON (`handshake.descriptor_commit_hex`, `pow.*`) Negro
  اٹھائے گئے موثر PoW/parámetros de rompecabezas واپس کرتا ہے۔
- `POST /v2/puzzle/mint` - Billete Argon2 nuevo کرتا ہے؛ cuerpo JSON opcional
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  کم TTL کی درخواست کرتا ہے (ventana de política تک abrazadera), ticket کو transcripción hash
  سے enlazar کرتا ہے، اور claves de firma configuradas ہوں تو boleto firmado por retransmisión +
  firma huella digital واپس کرتا ہے۔
- `GET /v2/token/config` - Aquí `pow.token.enabled = true` y un token de admisión activo
  política واپس کرتا ہے (huella digital del emisor, límites TTL/desviación del reloj, ID de retransmisión, اور
  conjunto de revocación fusionado).
- `POST /v2/token/mint` - Token de admisión ML-DSA nuevo کرتا ہے جو hash de currículum proporcionado
  سے obligado ہوتا ہے؛ cuerpo de la solicitud `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`
  قبول کرتا ہے۔Servicio کے بنائے گئے tickets کو prueba de integración
`volumetric_dos_soak_preserves_puzzle_and_latency_slo` میں verificar کیا جاتا ہے، جو
escenarios volumétricos de DoS کے دوران aceleradores de relé بھی ejercicio کرتا ہے۔
【herramientas/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Configuración de emisión de tokens کرنا

`pow.token.*` کے تحت campos JSON de relé establecidos کریں (مثال کے لئے
`tools/soranet-relay/deploy/config/relay.entry.json` دیکھیں) تاکہ tokens ML-DSA
habilitar ہوں۔ کم از کم clave pública del emisor اور lista de revocación opcional فراہم کریں:

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

Servicio de rompecabezas انہی valores کو reutilización کرتا ہے اور runtime میں Norito Revocación JSON
فائل کو خودکار طور پر recargar کرتا ہے۔ CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) استعمال کریں تاکہ
tokens fuera de línea menta/inspeccionar ہوں، revocación فائل میں `token_id_hex` entradas anexar ہوں،
اور actualizaciones de producción سے پہلے موجودہ auditoría de credenciales ہوں۔

Clave secreta del emisor کو Banderas CLI کے ذریعے servicio de rompecabezas میں پاس کریں:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` بھی دستیاب ہے جب canalización secreta de herramientas fuera de banda کے ذریعے administrar ہو۔
Vigilante de archivos de revocación `/v2/token/config` کو actual رکھتا ہے؛ actualizaciones کو
`soranet-admission-token revoke` کمانڈ کے ساتھ coordinar کریں تاکہ estado de revocación
retraso نہ کرے۔Relay JSON میں `pow.signed_ticket_public_key_hex` set کریں تاکہ tickets PoW firmados
verificar کرنے کے لئے ML-DSA-44 publicidad de clave pública ہو؛ `/v2/puzzle/config` یہ clave اور
اس کا BLAKE3 huella digital (`signed_ticket_public_key_fingerprint_hex`) eco کرتا ہے تاکہ
PIN del verificador de clientes کر سکیں۔ ID de retransmisión de boletos firmados y enlaces de transcripción کے خلاف
validar ہوتے ہیں اور اسی revocación almacenar کو compartir کرتے ہیں؛ tickets PoW sin procesar de 74 bytes
verificador de ticket firmado configurado ہونے پر بھی válido رہتے ہیں۔ Secreto del firmante
`--signed-ticket-secret-hex` یا `--signed-ticket-secret-path` کے ذریعے lanzamiento del servicio
پر pasar کریں؛ Inicio pares de claves no coincidentes rechazar کرتا ہے اگر secreto
`pow.signed_ticket_public_key_hex` کے خلاف validar نہ ہو۔ `POST /v2/puzzle/mint`
`"signed": true` (اور opcional `"transcript_hash_hex"`) قبول کرتا ہے تاکہ Norito codificado
bytes del billete sin procesar del billete firmado کے ساتھ واپس ہو؛ respuestas میں `signed_ticket_b64`
اور `signed_ticket_fingerprint_hex` شامل ہوتے ہیں تاکہ reproducir pista de huellas dactilares ہوں۔
` signed = true` y solicitudes rechazadas ہوتی ہیں اگر secreto del firmante configurado نہ ہو۔

## Guía de rotación de claves1. **compromiso del descriptor نیا جمع کریں۔** Paquete de directorio de gobernanza میں retransmisión
   descriptor confirmar publicar کرتی ہے۔ Cadena hexadecimal کو relé Configuración JSON میں
   `handshake.descriptor_commit_hex` میں copia کریں جو servicio de rompecabezas کے ساتھ compartido ہے۔
2. **Revisión de los límites de la política de rompecabezas کریں۔** تصدیق کریں کہ actualizado
   Plan de lanzamiento de valores `pow.puzzle.{memory_kib,time_cost,lanes}` کے مطابق ہیں۔ Operadores کو
   Relés de configuración Argon2 میں determinista رکھنا چاہئے (کم از کم 4 MiB de memoria،
   1 <= carriles <= 16).
3. **Etapa de reinicio کریں۔** El corte de rotación de gobernanza anuncia کرے تو unidad systemd یا
   recarga de contenedores کریں۔ Servicio میں recarga en caliente نہیں ہے؛ نیا descriptor commit لینے کے لئے
   reiniciar ضروری ہے۔
4. **Validar کریں۔** `POST /v2/puzzle/mint` کے ذریعے emisión de billete کریں اور تصدیق کریں کہ
   `difficulty` اور `expires_at` Política de coincidencia ہوں۔ Informe de remojo
   (`docs/source/soranet/reports/pow_resilience.md`) referencia a los límites de latencia esperados
   capturar کرتا ہے۔ Los tokens permiten que ہوں تو `/v2/token/config` busque کریں تاکہ emisor anunciado
   huella digital اور recuento de revocación valores esperados سے coincidencia ہوں۔

## Procedimiento de desactivación de emergencia1. Configuración del relé compartido میں `pow.puzzle.enabled = false` set کریں۔
   `pow.required = true` رکھیں اگر hashcash fallback tickets لازمی رہنے چاہئیں۔
2. Las entradas opcionales طور پر `pow.emergency` aplican la puerta کریں تاکہ Argon2 fuera de línea ہونے پر
   los descriptores obsoletos rechazan ہوں۔
3. Retransmisión اور servicio de rompecabezas دونوں reiniciar کریں تاکہ تبدیلی aplicar ہو۔
4. Monitor `soranet_handshake_pow_difficulty` کریں تاکہ dificultad valor hashcash esperado
   تک soltar ہو، اور verificar کریں کہ `/v2/puzzle/config` `puzzle = null` رپورٹ کرے۔

## Monitoreo y alerta- **Latencia SLO:** `soranet_handshake_latency_seconds` pista کریں اور P95 کو 300 ms سے نیچے رکھیں۔
  La prueba de remojo compensa los aceleradores de protección کے لئے datos de calibración فراہم کرتے ہیں۔
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Presión de cuota:** `soranet_guard_capacity_report.py` کو métricas de relé کے ساتھ استعمال کریں تاکہ
  Ajuste de tiempos de reutilización de `pow.quotas` (`soranet_abuse_remote_cooldowns`, `soranet_handshake_throttled_remote_quota_total`) ہوں۔
  【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alineación del rompecabezas:** `soranet_handshake_pow_difficulty` کو `/v2/puzzle/config` سے واپس ہونے والی
  dificultad کے ساتھ partido ہونا چاہئے۔ Configuración de retransmisión obsoleta de divergencia یا reinicio fallido کی نشاندہی ہے۔
- **Preparación del token:** اگر `/v2/token/config` غیر متوقع طور پر `enabled = false` ہو جائے یا
  `revocation_source` marcas de tiempo obsoletas رپورٹ کرے تو alerta کریں۔ Operadores کو CLI کے ذریعے Norito
  archivo de revocación rotar کرنا چاہئے جب کوئی token retirar ہو تاکہ یہ punto final درست رہے۔
- **Estado del servicio:** `/healthz` کو معمول کی cadencia de vida پر sonda کریں اور alerta کریں اگر
  `/v2/puzzle/mint` Respuestas HTTP 500 دے (discordancia de parámetros de Argon2 یا fallas de RNG کی نشاندہی).
  Errores de acuñación de tokens `/v2/token/mint` en respuestas HTTP 4xx/5xx کے ذریعے نظر آتے ہیں؛ fracasos repetidos
  کو condición de paginación سمجھیں۔

## Cumplimiento y registro de auditoríaLos eventos de relé estructurados `handshake` emiten razones de aceleración y duraciones de tiempo de reutilización.
یقینی بنائیں کہ `docs/source/soranet/relay_audit_pipeline.md` میں بیان کردہ canalización de cumplimiento ان registros کو ingesta کرے
تاکہ cambios en la política de rompecabezas auditables رہیں۔ Habilitación de puerta de rompecabezas, muestras de boletos acuñados y configuración Norito
instantánea کو ticket de lanzamiento کے ساتھ archivo کریں تاکہ مستقبل کے auditorías کے لئے دستیاب ہوں۔ Ventanas de mantenimiento سے پہلے
mint کئے گئے tokens de admisión کو ان کے `token_id_hex` valores کے ساتھ track کیا جانا چاہئے اور expirar یا revocar ہونے
پر archivo de revocación میں insertar کیا جانا چاہئے۔