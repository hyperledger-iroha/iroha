---
lang: es
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T15:38:30.658233+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Aprobación de la implementación de la carga útil v1 (Consejo SDK, 28 de abril de 2026).
//!
//! Captura el memorando de decisión del Consejo SDK requerido por `roadmap.md:M1` para que el
//! La implementación de la carga útil cifrada v1 tiene un registro auditable (entregable M1.4).

# Decisión de implementación de Payload v1 (2026-04-28)

- **Presidente:** Líder del Consejo SDK (M. Takemiya)
- **Miembros votantes:** Líder de Swift, Mantenedor de CLI, Activos confidenciales TL, DevRel WG
- **Observadores:** Gestión de programas, operaciones de telemetría

## Entradas revisadas

1. **Enlaces y remitentes rápidos**: `ShieldRequest`/`UnshieldRequest`, remitentes asíncronos y ayudantes del generador de Tx obtuvieron pruebas de paridad y docs.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **Ergonomía CLI**: el asistente `iroha app zk envelope` cubre flujos de trabajo de codificación/inspección además de diagnósticos de fallas, alineado con el requisito de ergonomía de la hoja de ruta.【crates/iroha_cli/src/zk.rs:1256】
3. **Dispositivos deterministas y conjuntos de paridad**: dispositivo compartido + validación Rust/Swift para mantener bytes/superficies de error Norito alineado.【fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## Decisión

- **Aprobar la implementación de la carga útil v1** para SDK y CLI, lo que permite que las billeteras Swift generen sobres confidenciales sin necesidad de tuberías personalizadas.
- **Condiciones:** 
  - Mantener los dispositivos de paridad bajo alertas de deriva de CI (vinculados a `scripts/check_norito_bindings_sync.py`).
  - Documentar el manual operativo en `docs/source/confidential_assets.md` (ya actualizado a través de Swift SDK PR).
  - Registre la evidencia de calibración y telemetría antes de activar cualquier indicador de producción (seguido en M2).

## Elementos de acción

| Propietario | Artículo | Vencimiento |
|-------|------|-----|
| Plomo rápido | Anunciar disponibilidad de GA + fragmentos README | 2026-05-01 |
| Mantenedor de CLI | Agregue el asistente `iroha app zk envelope --from-fixture` (opcional) | Trabajo pendiente (sin bloqueo) |
| Grupo de Trabajo sobre Desarrollo | Actualizar los inicios rápidos de la billetera con instrucciones de carga útil v1 | 2026-05-05 |

> **Nota:** Este memorando reemplaza la mención temporal de “aprobación pendiente del consejo” en `roadmap.md:2426` y satisface el elemento de seguimiento M1.4. Actualice `status.md` cada vez que se cierren los elementos de acción de seguimiento.