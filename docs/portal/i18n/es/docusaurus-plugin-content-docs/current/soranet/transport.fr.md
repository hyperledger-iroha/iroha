---
lang: es
direction: ltr
source: docs/portal/docs/soranet/transport.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: transporte
título: Vista del conjunto del transporte SoraNet
sidebar_label: Vista del conjunto de transporte
descripción: Apretón de manos, rotación de sales y guía de capacidades para la superposición anónima de SoraNet.
---

:::nota Fuente canónica
Esta página refleja la especificación de transporte SNNet-1 en `docs/source/soranet/spec.md`. Gardez les deux copys alignees jusqu'a la retraite de l'ancien ensemble dedocumentation.
:::

SoraNet es una superposición anónima que busca las capturas de rango de SoraFS, la transmisión RPC de Norito y las líneas de datos futuras de Nexus. El transporte de programas (elementos de hoja de ruta **SNNet-1**, **SNNet-1a** y **SNNet-1b**) define un apretón de manos determinista, una negociación de capacidades post-cuánticas (PQ) y un plan de rotación de sales para que cada retransmisión, cliente y puerta de enlace observen la postura de seguridad.

## Investigación de objetos y modelos- Construya circuitos a tres pasos (entrada -> medio -> salida) en QUIC v1 para que los pares no abusen nunca de la dirección Torii.
- Superposición de un apretón de manos Ruido XX *híbrido* (Curve25519 + Kyber768) junto con QUIC/TLS para colocar las claves de sesión en la transcripción TLS.
- Exiger des TLVs de capacite qui annoncent le support PQ KEM/signature, le role du Relay et la versión de protocolo; GREASE tipos de problemas desconocidos para las extensiones futuras que se pueden implementar.
- Rotación diaria de sales de contenido aveugle y epinglage des guard Relays durante 30 días para que la rotación del directorio no pueda desanonimizar a los clientes.
- Garder des cell fixes a 1024 B, inyector du padding/des cell dummy y exportador de una telemetría determinante para detectar rápidamente las tentativas de degradación.

## Canalización del protocolo de enlace (SNNet-1a)

1. **Sobre QUIC/TLS**: los clientes se conectan a relés auxiliares a través de QUIC v1 y terminan un protocolo de enlace TLS 1.3 utilizando los certificados Ed25519 firmados por la CA de gobierno. El exportador TLS (`tls-exporter("soranet handshake", 64)`) alimenta el ruido del sofá para que las transcripciones sean inseparables.
2. **Híbrido de ruido XX** - cadena de protocolo `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` con prólogo = exportador TLS. Flujo de mensajes:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```La salida DH Curve25519 y las dos encapsulaciones Kyber son melangees en las cuerdas simétricas finales. Un control de negociación PQ anula la finalización del protocolo de enlace: no se autoriza ningún tipo de retroceso clásico.

3. **Tickets y tokens de rompecabezas**: los relés pueden exigir un ticket de prueba de trabajo Argon2id antes de `ClientHello`. Les tickets sont des frames prefixees par la longueur qui transportent la solution Argon2 hachee et expirent dans les bornes de la politique:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Los tokens de prefijos de admisión par `SNTK` contienen los rompecabezas después de la firma ML-DSA-44 del emisor válido contra la política activa y la lista de revocación.

4. **Intercambio de capacidad TLV** - la carga útil final de ruido transporta los TLV de capacidad decrits ci-dessous. Los clientes abandonan la conexión si una capacidad obligatoria (PQ KEM/firma, rol o versión) se mantiene o en conflicto con la entrada del directorio.

5. **Diario de transcripción**: los relés periodísticos del hash de la transcripción, el uso de TLS y el contenido TLV para alimentar los detectores de degradación y las canalizaciones de conformidad.

## TLV de capacidad (SNNet-1c)

Capacidades reutilizables de un sobre TLV fijo `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Tipos define aujourd'hui:- `snnet.pqkem` - nivel Kyber (`kyber768` para el lanzamiento actual).
- `snnet.pqsig` - suite de firma PQ (`ml-dsa-44`).
- `snnet.role` - función del relé (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - Identificador de la versión del protocolo.
- `snnet.grease` - entradas de repuestos aleatorios en la playa reservada para garantizar la tolerancia de futuros TLV.

Los clientes mantienen una lista de TLV permitidos requeridos y hacen eco del protocolo de enlace cuando los protocolos de enlace son omitidos o degradados. Los relés publican el meme configurado en el microdescriptor de directorio para que la validación sea determinante.

## Rotación de sales y cegamiento CID (SNNet-1b)

- La gobernanza publica un registro `SaltRotationScheduleV1` con los valores `(epoch_id, salt, valid_after, valid_until)`. Los relés y puertas de enlace recuperan la firma del calendario después del editor del directorio.
- Les client appliquent le nouveau salt a `valid_after`, conservan le salt precedente pour un periode de Grace de 12 h et gardent un historique de 7 epoques pour tolerar les mises a jour retardees.
- Les identifiants aveugles canonices utilisent:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```Les gateways aceptan la cle aveugle vía `Sora-Req-Blinded-CID` y la renvoient dans `Sora-Content-CID`. El circuito/solicitud de cegamiento (`CircuitBlindingKey::derive`) está disponible en `iroha_crypto::soranet::blinding`.
- Si un relé mantiene una época, el circuito de nuevos circuitos simplemente telecarga el calendario y emite un `SaltRecoveryEventV1`, que los tableros de control transmiten una señal de buscapersonas.

## Donnees de directorio y política de guardia

- Los microdescriptores indican la identidad del relé (Ed25519 + ML-DSA-65), las claves PQ, los TLV de capacidad, las etiquetas de región, la guardia de elegibilidad y la época de sal anunciada.
- Los clientes guardan conjuntos de guardia durante 30 días y los cachés persistentes `guard_set` aux cotes du snapshot signe du directorio. Los wrappers CLI y SDK exponen el uso de caché después de que las preferencias de implementación se adjuntan a las revisiones de cambio.

## Telemetría y lista de verificación de implementación- Métricas exportadoras de producción avanzada:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Las señales de alerta viven en las costas de la matriz SLO del SOP de rotación de sales (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) y deben reflejarse en Alertmanager antes de la promoción del resultado.
- Alertas: >5 % de taux d'echec sur 5 minutos, retraso salino >15 minutos, o desajustes de capacidades observados en producción.
- Etapa de lanzamiento:
  1. Realice las pruebas de interoperabilidad de relé/cliente en preparación con el handshake híbrido y la pila PQ activa.
  2. Repetir el SOP de rotación de sales (`docs/source/soranet_salt_plan.md`) y unir los artefactos de perforación al expediente de cambio.
  3. Active la negociación de capacidades en el directorio, luego despliegue los relés de entrada, los relés intermediarios, los relés de salida y finalmente los clientes.
  4. Registre los paneles de caché de guardia, los calendarios de sal y los paneles de telemetría para cada fase; joindre le bundle de preuves a `status.md`.

Esta lista de verificación permite a los equipos de operadores, clientes y SDK adoptar los transportes SoraNet de acuerdo con el determinismo y las exigencias de auditoría capturadas en la hoja de ruta SNNet.