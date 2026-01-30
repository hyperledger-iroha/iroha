---
lang: es
direction: ltr
source: docs/source/soranet_handshake_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb35136774a013bfdfac4d33c6d51b6b4648f20c958c3b3a7d98e3186d0a3782
source_last_modified: "2025-11-02T04:40:40.022116+00:00"
translation_last_reviewed: "2026-01-30"
---

# Plan de handshake QUIC/Noise de SoraNet (Borrador)

## Objetivos
- Decidir si superponer Noise sobre QUIC o usar QUIC/TLS 1.3 con suites PQ.
- Documentar el enlace del transcript, el comportamiento de reanudación y la detección de downgrade.
- Publicar notas de auditoría para implementadores de relay/cliente.

## Temas
- Hash del vector de capacidades dentro del mensaje Finished.
- Extracción Dual-KDF para entradas clásicas + PQ.
- Políticas de tickets/reanudación.
- TLVs de capacidad GREASE.

## Matriz comparativa de transporte

| Aspecto | QUIC + Noise XX | QUIC + TLS 1.3 (suites PQ) |
|---------|------------------|----------------------------|
| Preparación PQ | Integración nativa de ML-KEM-768/1024 y Dilithium3; sin dependencia de las hojas de ruta de bibliotecas TLS. | Requiere stacks TLS con PQ; las suites híbridas siguen siendo experimentales en OpenSSL/BoringSSL. |
| Control del transcript | Hash del transcript definido por nosotros; incluye TLVs de capacidades/GREASE para resistencia a downgrades. | TLS Finished cubre extensiones estándar; TLVs personalizados requieren extensiones adicionales y validación compleja. |
| Esfuerzo de implementación | Framing Noise personalizado dentro de un stream QUIC; comportamiento determinista bajo nuestro control. | Usa bibliotecas TLS existentes, pero necesita parches para suites PQ, ECH y padding determinista. |
| Interop y pruebas | El harness puede generar vectores canónicos; menos diferencias entre librerías. | El comportamiento varía por implementación TLS, lo que dificulta fixtures reproducibles. |
| Riesgo de osificación | GREASE + TLVs personalizados mantenidos por la especificación. | Middleboxes TLS pueden bloquear nuevas extensiones; GREASE limitado a lo que TLS permite. |
| Decisión | Adoptar como transporte principal (resolución del WG 2026‑02‑12). | Se mantiene como respaldo de auditoría hasta que TLS PQ madure. |

## Feedback de revisión criptográfica (Feb 2026)

- Revisores: Crypto WG, Networking WG, consultor externo de PQ.
- Resultados:
  1. Dual-KDF (`HKDF(clásico || pq, transcript_hash)`) confirmado seguro; el hash del transcript debe incluir el orden de TLVs y los bytes GREASE (ya documentado).
  2. ML-KEM-768 obligatorio; ML-KEM-1024 opcional. Clientes solo clásicos deben disparar alarmas de downgrade y rechazo antes del KDF.
  3. Firmas Dilithium3 requeridas para descriptores/anuncios de salt; testigo Ed25519 se mantiene para tooling.
  4. El fallback TLS se permite solo para rutas de auditoría con reanudación de sesión deshabilitada hasta que TLS PQ se estabilice.
  5. Acciones: publicar vectores de prueba del handshake (baseline/downgrade/GREASE), documentar el procedimiento de auditoría del fallback TLS, asegurar que el harness cubra la telemetría de downgrade.

Feedback registrado en las minutas del WG; este plan ya refleja la línea base acordada.
