---
lang: es
direction: ltr
source: docs/portal/docs/soranet/transport.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: transporte
título: Descripción general del transporte SoraNet
sidebar_label: descripción general del transporte
descripción: Superposición de anonimato de SoraNet کے لئے apretón de manos, rotación de sal, اور guía de capacidad۔
---

:::nota Fuente canónica
یہ صفحہ `docs/source/soranet/spec.md` میں Especificación de transporte SNNet-1 کی عکاسی کرتا ہے۔ جب تک پرانا conjunto de documentación retirar نہ ہو، دونوں کاپیاں sincronización رکھیں۔
:::

SoraNet y superposición de anonimato, SoraFS y capturas de rango, transmisión RPC Norito, y líneas de datos Nexus. ہے۔ programa de transporte (elementos de la hoja de ruta **SNNet-1**, **SNNet-1a**, y **SNNet-1b**) نے apretón de manos determinista, negociación de capacidad post-cuántica (PQ), اور plan de rotación de sal متعین کیا تاکہ ہر retransmisión, cliente, اور puerta de enlace ایک ہی observación de la postura de seguridad کرے۔

## Objetivos y modelo de red- QUIC v1 پر circuitos de tres saltos (entrada -> medio -> salida) بنانا تاکہ pares abusivos کبھی Torii تک براہ راست نہ پہنچیں۔
- QUIC/TLS کے اوپر Ruido XX *híbrido* apretón de manos (Curve25519 + Kyber768) capas تاکہ claves de sesión Transcripción TLS سے enlazar ہوں۔
- capacidad TLV لازمی کریں جو PQ KEM/soporte de firma, función de retransmisión, اور versión del protocolo anunciar کریں؛ tipos desconocidos کو GREASE کریں تاکہ futuras extensiones desplegables رہیں۔
- sales de contenido cegado کو روزانہ rotar کریں اور relés de guardia کو 30 دن pin کریں تاکہ directorio de clientes de abandono کو desanonimizar نہ کر سکے۔
- celdas کو 1024 B پر fijo رکھیں، relleno/celdas ficticias inyectar کریں، اور telemetría determinista exportación کریں تاکہ intentos de degradación جلد پکڑی جائیں۔

## Tubería de protocolo de enlace (SNNet-1a)

1. **Sobre QUIC/TLS** - clientes QUIC v1 پر relés سے conectar کرتے ہیں اور Ed25519 certificados کے ساتھ TLS 1.3 handshake complete کرتے ہیں جو gobernanza CA نے firmar کئے ہوتے ہیں۔ Exportador TLS (`tls-exporter("soranet handshake", 64)`) Capa de ruido کو semilla کرتا ہے تاکہ transcripciones inseparables رہیں۔
2. **Híbrido de ruido XX** - cadena de protocolo `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` con prólogo = exportador TLS۔ Flujo de mensajes:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Curve25519 Salida DH اور دونوں Encapsulaciones Kyber teclas simétricas finales میں mix ہوتے ہیں۔ Negociar material PQ نہ ہو تو apretón de manos مکمل طور پر abort ہوتا ہے - respaldo solo clásico کی اجازت نہیں۔3. **Boletos y fichas de rompecabezas** - relés `ClientHello` سے پہلے Argon2id ticket de prueba de trabajo مانگ سکتے ہیں۔ Marcos con prefijo de longitud de tickets ہیں جو hash Argon2 solución لے جاتے ہیں اور límites de política کے اندر expiran ہوتے ہیں:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Prefijo `SNTK` والے tokens de admisión omisión de acertijos کرتے ہیں جب emisor کی Firma ML-DSA-44 política activa اور lista de revocación کے خلاف validar ہو جائے۔

4. **Capacidad de intercambio de TLV** - carga útil de ruido final نیچے بیان کردہ Capacidad de TLV لے جاتا ہے۔ Falta la capacidad de اگر کوئی لازمی (PQ KEM/firma, rol, یا versión) ہو یا entrada de directorio سے falta de coincidencia کرے تو interrupción de la conexión del cliente کرتے ہیں۔

5. **Registro de transcripción**: transmite hash de transcripción, huella digital TLS, registro de contenidos TLV, detectores de degradación, canalizaciones de cumplimiento, feeds, etc.

## TLV de capacidad (SNNet-1c)

Capacidades ایک مقررہ `typ/length/value` Reutilización de sobres TLV کرتی ہیں:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Estos son tipos definidos:

- `snnet.pqkem` - Nivel Kyber (`kyber768` موجودہ implementación کے لئے).
- `snnet.pqsig` - Suite de firmas PQ (`ml-dsa-44`).
- `snnet.role` - función del relé (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identificador de versión del protocolo.
- `snnet.grease` - rango reservado میں entradas de relleno aleatorias تاکہ futuros TLV toleran ہوں۔Los clientes requirieron TLV کی lista de permitidos رکھتے ہیں اور faltantes یا degradación ہونے پر apretón de manos fallido کرتے ہیں۔ Relés یہی establecer اپنی directorio microdescriptor میں publicar کرتے ہیں تاکہ validación determinista رہے۔

## Rotación de sal y cegamiento de CID (SNNet-1b)

- Gobernanza `SaltRotationScheduleV1` publicación de registros کرتی ہے جس میں `(epoch_id, salt, valid_after, valid_until)` valores ہوتے ہیں۔ Retransmisiones اور puertas de enlace calendario firmado کو editor de directorio سے buscar کرتے ہیں۔
- Clientes `valid_after` پر نیا aplicación de sal کرتے ہیں، پچھلا sal 12 h período de gracia کے لئے رکھتے ہیں، اور actualizaciones retrasadas کے لئے 7 épocas de retención del historial کرتے ہیں۔
- Identificadores cegados canónicos یوں بنتے ہیں:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Puertas de enlace `Sora-Req-Blinded-CID` کے ذریعے llave ciega قبول کرتے ہیں اور `Sora-Content-CID` میں echo کرتے ہیں۔ Cegamiento de circuito/solicitud (`CircuitBlindingKey::derive`) `iroha_crypto::soranet::blinding` میں موجود ہے۔
- اگر relé کوئی época perdida کرے تو وہ نئے circuitos روک دیتا ہے جب تک programación descarga نہ کر لے اور `SaltRecoveryEventV1` emitir کرتا ہے، جسے Señal de localización de paneles de control de guardia کے طور پر tratar کرتے ہیں۔

## Datos del directorio y política de protección- Los microdescriptores retransmiten identidad (Ed25519 + ML-DSA-65), claves PQ, TLV de capacidad, etiquetas de región, elegibilidad de guardia, اور موجودہ época de sal anunciada لے جاتے ہیں۔
- Conjuntos de protección de clientes کو 30 دن pin کرتے ہیں اور `guard_set` cachés کو instantánea del directorio firmado کے ساتھ persist کرتے ہیں۔ CLI اور SDK contenedores caché superficie de huellas dactilares کرتے ہیں تاکہ implementación revisiones de cambios کے ساتھ adjuntar ہو سکے۔

## Lista de verificación de telemetría e implementación

- Métricas de producción y exportación:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Umbrales de alerta rotación de sal Matriz SOP SLO (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) کے ساتھ رہتی ہیں اور promoción de red کرنے سے پہلے Alertmanager میں mirror ہونی چاہئیں۔
- Alertas: 5 minutos >5 % de tasa de falla, retraso salino >15 minutos, y desajustes en la capacidad del sistema de producción.
- Pasos de implementación:
  1. Preparación del protocolo de enlace híbrido y habilitación de la pila PQ y pruebas de interoperabilidad de retransmisión/cliente.
  2. SOP de rotación de sal (`docs/source/soranet_salt_plan.md`) ensayar کریں اور artefactos de perforación cambiar registro کے ساتھ adjuntar کریں۔
  3. La negociación de la capacidad del directorio del directorio habilita los relés de entrada, los relés intermedios, los relés de salida, los clientes y el despliegue de los clientes.
  4. ہر fase کے لئے huellas dactilares de caché de guardia, horarios de sal, اور registros de paneles de telemetría کریں؛ paquete de pruebas کو `status.md` کے ساتھ adjuntar کریں۔یہ lista de verificación seguir کرنے سے operador، cliente، اور equipos SDK SoraNet transporta کو یکساں رفتار سے adoptar کر سکتے ہیں اور SNNet roadmap میں موجود determinismo اور requisitos de auditoría پوری کرتے ہیں۔