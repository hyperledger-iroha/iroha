---
lang: es
direction: ltr
source: docs/portal/docs/soranet/transport.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: transport
title: Resumen del transporte de SoraNet
sidebar_label: Resumen de transporte
description: Handshake, rotacion de salts y guia de capacidades para el overlay de anonimato de SoraNet.
---

:::note Fuente canonica
Esta pagina refleja la especificacion de transporte SNNet-1 en `docs/source/soranet/spec.md`. Manten ambas copias sincronizadas.
:::

SoraNet es el overlay de anonimato que respalda los range fetches de SoraFS, el streaming de Norito RPC y los futuros data lanes de Nexus. El programa de transporte (items del roadmap **SNNet-1**, **SNNet-1a** y **SNNet-1b**) definio un handshake determinista, negociacion de capacidades post-quantum (PQ) y un plan de rotacion de salts para que cada relay, client y gateway observe la misma postura de seguridad.

## Objetivos y modelo de red

- Construir circuitos de tres saltos (entry -> middle -> exit) sobre QUIC v1 para que los peers abusivos nunca lleguen a Torii directamente.
- Superponer un handshake Noise XX *hibrido* (Curve25519 + Kyber768) sobre QUIC/TLS para ligar las claves de sesion al transcript TLS.
- Requerir TLVs de capacidades que anuncien soporte PQ KEM/firma, rol del relay y version de protocolo; hacer GREASE de tipos desconocidos para que futuras extensiones sigan desplegables.
- Rotar salts de contenido ciego a diario y fijar guard relays por 30 dias para que el churn del directorio no pueda desanonimizar clientes.
- Mantener cells fijas de 1024 B, inyectar padding/celdas dummy y exportar telemetria determinista para que los intentos de downgrade se detecten rapido.

## Pipeline de handshake (SNNet-1a)

1. **QUIC/TLS envelope** - los clients se conectan a relays sobre QUIC v1 y completan un handshake TLS 1.3 usando certificados Ed25519 firmados por la governance CA. El TLS exporter (`tls-exporter("soranet handshake", 64)`) alimenta la capa Noise para que los transcripts sean inseparables.
2. **Noise XX hybrid** - string de protocolo `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` con prologue = TLS exporter. Flujo de mensajes:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   La salida DH de Curve25519 y ambas encapsulaciones Kyber se mezclan en las claves simetricas finales. Si no se negocia material PQ, el handshake se aborta por completo: no se permite fallback solo clasico.

3. **Puzzle tickets y tokens** - los relays pueden exigir un ticket de proof-of-work Argon2id antes de `ClientHello`. Los tickets son frames con prefijo de longitud que llevan la solucion Argon2 hasheada y expiran dentro de los limites de la politica:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Los tokens de admision prefijados con `SNTK` evitan los puzzles cuando una firma ML-DSA-44 del emisor valida contra la politica activa y la lista de revocacion.

4. **Intercambio de capability TLV** - el payload final de Noise transporta los TLVs de capacidades descritos abajo. Los clients abortan la conexion si cualquier capacidad obligatoria (PQ KEM/firma, rol o version) falta o no coincide con la entrada del directorio.

5. **Registro del transcript** - los relays registran el hash del transcript, la huella TLS y el contenido TLV para alimentar detectores de downgrade y pipelines de cumplimiento.

## Capability TLVs (SNNet-1c)

Las capacidades reutilizan un sobre TLV fijo de `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Tipos definidos hoy:

- `snnet.pqkem` - nivel Kyber (`kyber768` para el rollout actual).
- `snnet.pqsig` - suite de firma PQ (`ml-dsa-44`).
- `snnet.role` - rol del relay (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identificador de version de protocolo.
- `snnet.grease` - entradas de relleno aleatorias en el rango reservado para asegurar que futuros TLVs se toleren.

Los clients mantienen una allow-list de TLVs requeridos y fallan el handshake si se omiten o se degradan. Los relays publican el mismo set en su microdescriptor del directorio para que la validacion sea determinista.

## Rotacion de salts y CID blinding (SNNet-1b)

- Governance publica un registro `SaltRotationScheduleV1` con valores `(epoch_id, salt, valid_after, valid_until)`. Relays y gateways obtienen el calendario firmado desde el directory publisher.
- Los clients aplican el nuevo salt en `valid_after`, mantienen el salt anterior durante un periodo de gracia de 12 h y retienen un historial de 7 epocas para tolerar actualizaciones retrasadas.
- Los identificadores ciegos canonicos usan:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Los gateways aceptan la clave ciega via `Sora-Req-Blinded-CID` y la hacen echo en `Sora-Content-CID`. El blinding de circuit/request (`CircuitBlindingKey::derive`) vive en `iroha_crypto::soranet::blinding`.
- Si un relay pierde una epoca, detiene nuevos circuits hasta que descargue el calendario y emite un `SaltRecoveryEventV1`, que los dashboards de guardia tratan como una senal de paging.

## Datos de directorio y politica de guards

- Los microdescriptores llevan identidad del relay (Ed25519 + ML-DSA-65), llaves PQ, TLVs de capacidades, etiquetas de region, elegibilidad de guard y la epoca de salt anunciada.
- Los clients fijan sets de guard por 30 dias y persisten caches `guard_set` junto con el snapshot firmado del directorio. CLI y SDK exponen la huella del cache para que la evidencia de rollout se adjunte a revisiones de cambio.

## Telemetria y checklist de rollout

- Metricas a exportar antes de produccion:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Los umbrales de alerta viven junto a la matriz SLO de SOP de rotacion de salts (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) y deben reflejarse en Alertmanager antes de promocionar la red.
- Alertas: >5% de tasa de fallo en 5 minutos, salt lag >15 minutos o mismatches de capacidades observados en produccion.
- Pasos de rollout:
  1. Ejecutar pruebas de interoperabilidad relay/client en staging con el handshake hibrido y el stack PQ habilitados.
  2. Ensayar el SOP de rotacion de salts (`docs/source/soranet_salt_plan.md`) y adjuntar los artefactos del simulacro al registro de cambios.
  3. Habilitar la negociacion de capacidades en el directorio, luego desplegar a relays de entrada, relays intermedios, relays de salida y finalmente clients.
  4. Registrar fingerprints de cache de guard, calendarios de salt y dashboards de telemetria para cada fase; adjuntar el paquete de evidencia a `status.md`.

Seguir este checklist permite que equipos de operadores, clients y SDK adopten los transports de SoraNet al mismo ritmo mientras cumplen con la determinacion y los requisitos de auditoria capturados en el roadmap SNNet.
