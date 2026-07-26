---
lang: es
direction: ltr
source: docs/portal/docs/soranet/transport.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: transporte
título: Обзор транспорта SoraNet
sidebar_label: Objeto de transporte
descripción: Apretón de manos, rotación de sales y руководство по возможностям для анонимного оверлея SoraNet.
---

:::nota Канонический источник
Esta página contiene especificaciones técnicas de transporte SNNet-1 en `docs/source/soranet/spec.md`. Deje copias sincronizadas, ya que no contiene documentos exclusivos.
:::

SoraNet: este anónimo conjunto de rangos de capturas de datos SoraFS, Norito RPC streaming y líneas de datos adicionales Nexus. El transporte del programa (elementos de la hoja de ruta **SNNet-1**, **SNNet-1a** y **SNNet-1b**) incluye un protocolo de enlace determinado, enlaces a mensajes post-cuánticos (PQ) y план ротации sales, чтобы каждый relé, cliente y puerta de enlace наблюдал одинаковую postura de seguridad.

## Modelos y modelos- Строить треххоповые circuitos (entrada -> medio -> salida) поверх QUIC v1, чтобы злоупотребляющие peers никогда не достигали Torii напрямую.
- Наложить handshake Noise XX *hybrid* (Curve25519 + Kyber768) поверх QUIC/TLS, чтобы привязать claves de sesión con transcripción TLS.
- TLV de capacidad triple, которые объявляют поддержку PQ KEM/подписи, роль Relay y версию протокола; GREASE неизвестные типы, чтобы будущие расширения оставались развертываемыми.
- Ежедневно ротировать sales de contenido ciego y фиксировать guardias relés durante 30 días, чтобы churn в directorio не мог деанонимизировать клиентов.
- Eliminar las funciones de las celdas en 1024 B, agregar celdas rellenas/ficticias y exportar la telemetría determinada, y reducir la calidad de las opciones.

## Apretón de manos de canalización (SNNet-1a)

1. **Sobre QUIC/TLS**: los clientes utilizan los relés de QUIC v1 y almacenan el protocolo de enlace TLS 1.3, implementan las certificaciones Ed25519 y administran CA. Exportador TLS (`tls-exporter("soranet handshake", 64)`) засевает слой Ruido, чтобы transcripciones были неразделимы.
2. **Noise XX hybrid** - строка протокола `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` с prólogo = exportador TLS. Foto de archivo:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   El Curve25519 DH y las encapsulaciones Kyber se combinan con teclas simétricas finales. Los usuarios de material PQ pueden utilizar el protocolo de enlace - respaldo en la versión clásica.3. **Boletos y tokens de rompecabezas** - los relés pueden incluir el ticket de prueba de trabajo Argon2id de `ClientHello`. Entradas: estos fotogramas con prefijo de longitud, que no necesitan ninguna revisión de Argon2 y están instalados en políticas previas:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Los tokens de admisión con la especificación `SNTK` permiten resolver rompecabezas, junto con el ML-DSA-44 o la emisión de políticas activas activas. списка отзыва.

4. **Capacidad de intercambio de TLV**: la carga útil de ruido final permite TLV de capacidad, no disponible. Los clientes prefieren la capacidad, o tienen capacidad (PQ KEM/подпись, роль или версия) отсутствует или не совпадает с записью directorio.

5. **Registro de transcripción**: retransmite la transcripción hash, la huella digital TLS y el contenido TLV, permite detectar detectores de degradación y canalizaciones de cumplimiento.

## TLV de capacidad (SNNet-1c)

Capacidades utilizadas en el software TLV-оболочку `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Tipos de temporada predominantes:

- `snnet.pqkem` - уровень Kyber (`kyber768` для текущего rollout).
- `snnet.pqsig` - suite PQ подписей (`ml-dsa-44`).
- `snnet.role` - relé de polo (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - protocolo de versión del identificador.
- `snnet.grease` - Elementos de relleno exclusivos en la base de llenado, que deben cumplir con los TLV de volumen.Los clientes pueden incluir TLV en la lista de permitidos y validar el protocolo de enlace antes de realizar una degradación. Los relés se publican en el microdescriptor del directorio establecido, ya que son válidos según los parámetros.

## Sales de Rotación y cegamiento CID (SNNet-1b)

- Gobernanza publicada por `SaltRotationScheduleV1` y `(epoch_id, salt, valid_after, valid_until)`. Los relés y las puertas de enlace son una publicación gráfica de un editor de directorios.
- Los clientes presentan la nueva sal en `valid_after`, la versión anterior de la sal en 12 horas, el período de gracia y la historia en 7 épocas переносимости задержанных обновлений.
- Aplicación de identificadores ciegos canónicos:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Las puertas de enlace tienen una clave ciega en `Sora-Req-Blinded-CID` y también en `Sora-Content-CID`. Cegamiento de circuito/solicitud (`CircuitBlindingKey::derive`) находится в `iroha_crypto::soranet::blinding`.
- Esta era de transmisión de retransmisión, con nuevos circuitos integrados para gráficos y mensajes `SaltRecoveryEventV1`, paneles de control de guardia, tractocamiones y paginación. señal.

## Datos del directorio y política de protección

- Microdescriptores de relé de identidad (Ed25519 + ML-DSA-65), claves PQ, TLV de capacidad, etiquetas de región, elegibilidad de guardia y época de sal anunciada.
- Los clientes guardan en caché conjuntos de 30 días y los cachés `guard_set` se almacenan en una instantánea del directorio. Los envoltorios CLI y SDK incluyen huellas dactilares de caché y la implementación de pruebas puede bloquear la revisión de cambios.

## Telemetría y lista de verificación de implementación- Métricas para el deporte antes de la producción:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Пороговые значения алертов живут рядом с матрицей SLO для SOP ротации salts (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) y dolжны быть отражены в Alertmanager до продвижения сети.
- Alertas: tasa de fallas >5% cada 5 minutos, retraso salino >15 minutos o discrepancias de capacidad en producción.
- Lanzamiento de Shagi:
  1. Prognosticar las pruebas de interoperabilidad del relé/cliente en la preparación con un protocolo de enlace híbrido y una pila PQ.
  2. Controle las sales de rotación SOP (`docs/source/soranet_salt_plan.md`) y pruebe los artefactos de perforación para cambiar el registro.
  3. Abra la negociación de capacidades en el directorio, realice operaciones en los relés de entrada, los relés intermedios, los relés de salida y en varios clientes.
  4. Зафиксировать huellas dactilares de caché de guardia, horarios de sal y paneles de telemetría para cada fase; приложить paquete de pruebas к `status.md`.

Следование этому checklist позволяет командам операторов, clientes y SDK внедрять SoraNet transports sincronizados y при этом выполнять требования детерминизма Y auditamos, estamos en la hoja de ruta de SNNet.