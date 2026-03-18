---
lang: es
direction: ltr
source: docs/portal/docs/soranet/transport.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: transporte
título: Resumen del transporte de SoraNet
sidebar_label: Resumen de transporte
descripción: Apretón de manos, rotación de sales y guía de capacidades para la superposición de anonimato de SoraNet.
---

:::nota Fuente canónica
Esta página refleja la especificación de transporte SNNet-1 en `docs/source/soranet/spec.md`. Manten ambas copias sincronizadas.
:::

SoraNet es la superposición de anonimato que respalda los range fetches de SoraFS, el streaming de Norito RPC y los futuros data lanes de Nexus. El programa de transporte (elementos del roadmap **SNNet-1**, **SNNet-1a** y **SNNet-1b**) define un handshake determinista, negociación de capacidades post-quantum (PQ) y un plan de rotación de sales para que cada relevo, cliente y gateway observen la misma postura de seguridad.

## Objetivos y modelo de rojo- Construir circuitos de tres saltos (entrada -> medio -> salida) sobre QUIC v1 para que los pares abusivos nunca lleguen a Torii directamente.
- Superponer un handshake Noise XX *hibrido* (Curve25519 + Kyber768) sobre QUIC/TLS para ligar las claves de sesión al transcript TLS.
- Requerir TLVs de capacidades que anuncien soporte PQ KEM/firma, rol del relé y versión de protocolo; hacer GREASE de tipos desconocidos para que futuras extensiones sigan desplegables.
- Rotar sales de ciego contenido a diario y fijar guard Relays por 30 días para que el churn del directorio no pueda desanonimizar clientes.
- Mantener celdas fijas de 1024 B, inyectar padding/celdas dummy y exportar telemetría determinista para que los intentos de downgrade se detecten rápidamente.

## Canalización del protocolo de enlace (SNNet-1a)

1. **QUIC/TLS sobre** - los clientes se conectan a retransmisiones sobre QUIC v1 y completan un handshake TLS 1.3 usando certificados Ed25519 firmados por la gobernanza CA. El exportador TLS (`tls-exporter("soranet handshake", 64)`) alimenta la capa Noise para que las transcripciones sean inseparables.
2. **Noise XX hybrid** - cadena de protocolo `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` con prólogo = exportador TLS. Flujo de mensajes:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```La salida DH de Curve25519 y ambas encapsulaciones Kyber se mezclan en las claves simétricas finales. Si no se negocia material PQ, el apretón de manos se aborta por completo: no se permite retroceder solo clásico.

3. **Tickets y tokens de rompecabezas** - los relés pueden requerir un ticket de prueba de trabajo Argon2id antes de `ClientHello`. Los tickets son frames con prefijo de longitud que llevan la solución Argon2 hasheada y expiran dentro de los limites de la política:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Los tokens de admision prefijados con `SNTK` evitan los rompecabezas cuando una firma ML-DSA-44 del emisor valida contra la politica activa y la lista de revocacion.

4. **Intercambio de capacidad TLV** - la carga útil final de Ruido transporta los TLV de capacidades descritos abajo. Los clientes abortan la conexión si cualquier capacidad obligatoria (PQ KEM/firma, rol o versión) falta o no coincide con la entrada del directorio.

5. **Registro del transcript** - los relés registran el hash del transcript, la huella TLS y el contenido TLV para alimentar detectores de downgrade y pipelines de cumplimiento.

## TLV de capacidad (SNNet-1c)

Las capacidades reutilizan un sobre TLV fijo de `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Tipos definidos hoy:- `snnet.pqkem` - nivel Kyber (`kyber768` para el despliegue actual).
- `snnet.pqsig` - suite de firma PQ (`ml-dsa-44`).
- `snnet.role` - rol del relé (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identificador de versión de protocolo.
- `snnet.grease` - entradas de relleno aleatorios en el rango reservado para asegurar que futuros TLV se toleren.

Los clientes mantienen una lista de permitidos de TLV requeridos y fallan el apretón de manos si se omite o se degradan. Los relés publican el mismo conjunto en su microdescriptor del directorio para que la validación sea determinista.

## Rotación de sales y cegamiento CID (SNNet-1b)

- Gobernanza publica un registro `SaltRotationScheduleV1` con valores `(epoch_id, salt, valid_after, valid_until)`. Relays y gateways obtienen el calendario firmado desde el directorio editor.
- Los clientes aplican el nuevo salt en `valid_after`, mantienen el salt anterior durante un periodo de gracia de 12 h y retienen un historial de 7 epocas para tolerar actualizaciones retrasadas.
- Los identificadores ciegos canónicos usan:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```Los gateways aceptan la clave ciega vía `Sora-Req-Blinded-CID` y la hacen echo en `Sora-Content-CID`. El cegamiento de circuito/request (`CircuitBlindingKey::derive`) vive en `iroha_crypto::soranet::blinding`.
- Si un relé pierde una época, detiene nuevos circuitos hasta que descargue el calendario y emite un `SaltRecoveryEventV1`, que los tableros de guardia tratan como una señal de paginación.

## Datos de directorio y politica de guardias

- Los microdescriptores llevan identidad del relé (Ed25519 + ML-DSA-65), llaves PQ, TLVs de capacidades, etiquetas de región, elegibilidad de guardia y la época de sal anunciada.
- Los clientes fijan conjuntos de guardia por 30 días y persisten cachés `guard_set` junto con la instantánea firmada del directorio. CLI y SDK exponen la huella del caché para que la evidencia de implementación se adjunte a revisión de cambio.

## Telemetría y checklist de implementación- Métricas a exportar antes de producción:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Los umbrales de alerta viven junto a la matriz SLO de SOP de rotación de sales (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) y deben reflejarse en Alertmanager antes de promocionar la red.
- Alertas: >5% de tasa de fallo en 5 minutos, salt lag >15 minutos o desajustes de capacidades observados en producción.
- Pasos de implementación:
  1. Ejecutar pruebas de interoperabilidad relé/cliente en staging con el handshake hibrido y el stack PQ habilitados.
  2. Ensayar el SOP de rotación de sales (`docs/source/soranet_salt_plan.md`) y adjuntar los artefactos del simulacro al registro de cambios.
  3. Habilitar la negociación de capacidades en el directorio, luego desplegar a relés de entrada, relés intermedios, relés de salida y finalmente clientes.
  4. Registrador de huellas dactilares de caché de guardia, calendarios de sal y paneles de telemetría para cada fase; adjuntar el paquete de evidencia a `status.md`.

Seguir esta lista de verificación permite que equipos de operadores, clientes y SDK adopten los transportes de SoraNet al mismo ritmo mientras cumplen con la determinación y los requisitos de auditoría capturados en el roadmap SNNet.