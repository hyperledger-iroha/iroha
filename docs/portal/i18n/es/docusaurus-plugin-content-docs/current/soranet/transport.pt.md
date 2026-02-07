---
lang: es
direction: ltr
source: docs/portal/docs/soranet/transport.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: transporte
título: Visao general del transporte SoraNet
sidebar_label: Visao general de transporte
descripción: Apretón de manos, rotación de sales y guía de capacidades para la superposición de anonimato de SoraNet.
---

:::nota Fuente canónica
Esta página especifica el transporte SNNet-1 en `docs/source/soranet/spec.md`. Mantenha ambas como copias sincronizadas.
:::

SoraNet y la superposición de anonimato que sustenta las capturas de rango de SoraFS, la transmisión de Norito RPC y futuros carriles de datos de Nexus. El programa de transporte (elementos de hoja de ruta **SNNet-1**, **SNNet-1a** y **SNNet-1b**) define un apretón de manos determinístico, negociación de capacidades post-quantum (PQ) y un plano de rotación de sales para que cada relé, cliente y puerta de enlace observen una misma postura de seguridad.

## Metas y modelo de red- Construir circuitos de tres saltos (entrada -> medio -> salida) sobre QUIC v1 para que pares abusivos nunca alcancen Torii directamente.
- Sobrepor un apretón de manos Ruido XX *híbrido* (Curve25519 + Kyber768) ao QUIC/TLS para ligar chaves de sesión ao transcript TLS.
- Exigir TLV de capacidades que anuncian soporte PQ KEM/assinatura, papel do relé y versao de protocolo; aplique GREASE en tipos desconocidos para manter futuras extensoes implantaveis.
- Rotacionar sales de conteudo cego diariamente y fijar relés de guardia por 30 días para que o churn do directorio nao possa desanonimizar clientes.
- Manter cell fixas em 1024 B, inyectar padding/cells dummy y exportar telemetría determinística para capturar tentativas de downgrade rápidamente.

## Canalización del protocolo de enlace (SNNet-1a)

1. **sobre QUIC/TLS** - los clientes se conectan a relés a través de QUIC v1 y completan un protocolo de enlace TLS 1.3 usando certificados Ed25519 assinados pela Governance CA. El exportador TLS (`tls-exporter("soranet handshake", 64)`) semeia a camada Noise para que las transcripciones sean inseparadas.
2. **Noise XX hybrid** - cadena de protocolo `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` con prólogo = exportador TLS. Flujo de mensajes:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   El resultado DH de Curve25519 y ambas encapsulaciones de Kyber sao misturados nas chaves simetricas finas. Falhar na negociacao do material PQ aborta o handshake por completo - nao ha fallback apenas clásico.3. **Tickets y tokens de rompecabezas** - Los relés pueden requerir un ticket de prueba de trabajo Argon2id antes de `ClientHello`. Os tickets sao frames com prefixo de comprimento que carregam a solucao Argon2 hasheada e expiram dentro de los límites de la política:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Los tokens de admisión con el prefijo `SNTK` pulam os rompecabezas cuando una assinatura ML-DSA-44 del emisor valida contra una política activa y una lista de revocación.

4. **Troca de capacidad TLV** - o payload final do Noise transporta os TLVs de capacidades descritos a continuación. Los clientes abortam a conexao se qualquer capacidade obrigatoria (PQ KEM/assinatura, papel ou versao) estiver ausente ou divergente da entrada do directorio.

5. **Registro de transcripción**: transmite el registro del hash de la transcripción, una impresión TLS digital y un conteudo TLV para alimentar detectores de degradación y tuberías de cumplimiento.

## TLV de capacidad (SNNet-1c)

Como capacidades de reutilización de un sobre TLV fijo de `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Tipos definidos hoy:- `snnet.pqkem` - nivel Kyber (`kyber768` para el despliegue actual).
- `snnet.pqsig` - suite de assinatura PQ (`ml-dsa-44`).
- `snnet.role` - papel del relé (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identificador de versao do protocolo.
- `snnet.grease` - entradas de preenchimento aleatorios na faixa reservada para garantizar tolerancia a futuros TLV.

Los clientes mantienen una lista permitida de TLV requeridos y faltan el apretón de manos cuando se omiten o se degradan. Los relés publican el mismo conjunto en su microdescriptor del directorio para que se valide de forma determinística.

## Rotacao de salts e CID blinding (SNNet-1b)

- A Governance Publica um registro `SaltRotationScheduleV1` com valores `(epoch_id, salt, valid_after, valid_until)`. Relés y puertas de enlace buscam o calendario assinado sin editor de directorio.
- Clients aplicam o novo salt em `valid_after`, mantem o salt anterior por um periodo de gracia de 12 h e retem um historico de 7 epochs para tolerar atualizacoes atrasadas.
- Identificadores cegos canónicos usados:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" || salt || cid)
  ```

  Las puertas de enlace aceitam a chave cega via `Sora-Req-Blinded-CID` y hacen eco en `Sora-Content-CID`. O cegamiento de circuito/solicitud (`CircuitBlindingKey::derive`) fica en `iroha_crypto::soranet::blinding`.
- Si un relé pierde una época, ele interrumpe nuevos circuitos al bajar el calendario y emite un `SaltRecoveryEventV1`, que los paneles de control de guardia tratan como señal de paginación.## Datos del directorio y política de guardia

- Microdescriptores carregam identidade do Relay (Ed25519 + ML-DSA-65), chaves PQ, TLVs de capacidades, tags de regiao, elegibilidade de guard e a epoch de salt anunciada.
- Los clientes reparan conjuntos de protección de 30 días y cachés persistentes `guard_set` junto con la instantánea asociada al directorio. Wrappers CLI y SDK incluyen la huella digital del caché para que a evidencia de implementación seja anexada a revisiones de mudanca.

## Telemetría y lista de verificación de implementación- Métricas a exportar antes de la producción:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Los límites de alerta viven al lado de la matriz SLO do SOP de rotación de sales (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) y deben ser refletidos no Alertmanager antes de la promoción de la red.
- Alertas: taxa de falha >5% em 5 minutos, salt lag >15 minutos, ou desajustes de capacidades observados en producción.
- Pasos de implementación:
  1. Ejecutar pruebas de interoperabilidad de retransmisión/cliente en staging con el handshake híbrido y el stack PQ habilitado.
  2. Ensaiar o SOP de rotacao de salts (`docs/source/soranet_salt_plan.md`) y anexar os artefatos do drill ao change record.
  3. Habilitar a negociacao de capacidades no directorio, depois rolar para relés de entrada, relés intermedios, relés de salida y por fim clientes.
  4. Huellas dactilares del caché de guardia del registrador, horarios de sal y paneles de telemetría para cada fase; anexar o paquete de pruebas a `status.md`.

Seguir esta lista de verificación permite que los tiempos de operadores, clientes y SDK adopten los transportes de SoraNet en conjunto en cuanto atendem a los requisitos de determinismo y auditorías capturadas en la hoja de ruta SNNet.