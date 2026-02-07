---
lang: es
direction: ltr
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T15:38:30.658859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Manual de estrategia de rotación de activos confidencial al que se hace referencia en `roadmap.md:M3`.

# Runbook confidencial de rotación de activos

Este manual explica cómo los operadores programan y ejecutan activos confidenciales.
rotaciones (conjuntos de parámetros, claves de verificación y transiciones de políticas) mientras
garantizar que las billeteras, los clientes Torii y los protectores de mempool sigan siendo deterministas.

## Ciclo de vida y estados

Conjuntos de parámetros confidenciales (`PoseidonParams`, `PedersenParams`, claves de verificación)
celosía y ayudante utilizados para derivar el estado efectivo a una altura determinada viven en
`crates/iroha_core/src/state.rs:7540`–`7561`. Barrido de ayudantes en tiempo de ejecución pendiente
transiciones tan pronto como se alcanza la altura objetivo y registra fallas para más adelante
retransmisiones (`crates/iroha_core/src/state.rs:6725`–`6765`).

Las políticas de activos incorporan
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
para que la gobernanza pueda programar actualizaciones a través de
`ScheduleConfidentialPolicyTransition` y cancélelos si es necesario. Ver
`crates/iroha_data_model/src/asset/definition.rs:320` y los espejos DTO Torii
(`crates/iroha_torii/src/routing.rs:1539`–`1580`).

## Flujo de trabajo de rotación

1. **Publicar nuevos paquetes de parámetros.** Los operadores envían
   Instrucciones `PublishPedersenParams`/`PublishPoseidonParams` (CLI
   `iroha app zk params publish ...`) para preparar nuevos grupos electrógenos con metadatos,
   ventanas de activación/desuso y marcadores de estado. El ejecutor rechaza
   ID duplicados, versiones que no aumentan o transiciones de estado incorrectas por
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499`–`2635`, y el
   Las pruebas de registro cubren los modos de falla (`crates/iroha_core/tests/confidential_params_registry.rs:93` – `226`).
2. **Actualizaciones de registro/verificación de claves.** `RegisterVerifyingKey` aplica el backend,
   compromiso y restricciones de circuito/versión antes de que una clave pueda ingresar al
   registro (`crates/iroha_core/src/smartcontracts/isi/world.rs:2067` – `2137`).
   La actualización de una clave desaproba automáticamente la entrada anterior y borra los bytes en línea,
   ejercido por `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1`.
3. **Programar transiciones de políticas de activos.** Una vez que los nuevos ID de parámetros estén activos,
   llamadas de gobierno `ScheduleConfidentialPolicyTransition` con el deseado
   modo, ventana de transición y hash de auditoría. El albacea se niega en conflicto.
   transiciones o activos con oferta transparente sobresaliente. Pruebas como
   `crates/iroha_core/tests/confidential_policy_gates.rs:300`–`384` verifique que
   las transiciones abortadas borran `pending_transition`, mientras que
   `confidential_policy_transition_reaches_shielded_only_on_schedule` en
   Las líneas 385–433 confirman que las actualizaciones programadas cambian a `ShieldedOnly` exactamente en
   la altura efectiva.
4. **Aplicación de políticas y protección de mempool.** El ejecutor de bloques barre todos los pendientes
   transiciones al inicio de cada bloque (`apply_policy_if_due`) y emite
   telemetría si una transición falla para que los operadores puedan reprogramarla. Durante la admisión
   el mempool rechaza transacciones cuya política efectiva cambiaría a mitad del bloque,
   garantizar la inclusión determinista a lo largo de la ventana de transición
   (`docs/source/confidential_assets.md:60`).

## Requisitos de billetera y SDK- Swift y otros SDK móviles exponen los ayudantes Torii para recuperar la política activa
  además de cualquier transición pendiente, para que las billeteras puedan advertir a los usuarios antes de firmar. Ver
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) y los asociados
  pruebas en `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591`.
- La CLI refleja los mismos metadatos a través de `iroha ledger assets data-policy get` (ayudante en
  `crates/iroha_cli/src/main.rs:1497`–`1670`), permitiendo a los operadores auditar el
  ID de política/parámetro conectados a una definición de activo sin desvelar el
  tienda de bloques.

## Cobertura de pruebas y telemetría

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288`–`345` verifica esa política
  las transiciones se propagan en instantáneas de metadatos y se borran una vez aplicadas.
- `crates/iroha_core/tests/zk_dedup.rs:1` demuestra que el caché `Preverify`
  rechaza gastos dobles/pruebas dobles, incluidos escenarios de rotación en los que
  los compromisos difieren.
- `crates/iroha_core/tests/zk_confidential_events.rs` y
  `zk_shield_transfer_audit.rs` cubrir escudo de extremo a extremo → transferir → desproteger
  fluye, asegurando que la pista de auditoría sobreviva a través de las rotaciones de parámetros.
- `dashboards/grafana/confidential_assets.json` y
  `docs/source/confidential_assets.md:401` documenta el árbol de compromisos y
  medidores de caché de verificador que acompañan a cada ejecución de calibración/rotación.

## Propiedad del runbook

- **DevRel / Wallet SDK Leads:** mantiene fragmentos de SDK + inicios rápidos que muestran
  cómo sacar a la superficie las transiciones pendientes y reproducir la menta → transferir → revelar
  pruebas localmente (seguidas bajo `docs/source/project_tracker/confidential_assets_phase_c.md:M3.2`).
- **Gestión de programas/Activos confidenciales TL:** aprobar solicitudes de transición, mantener
  `status.md` actualizado con las próximas rotaciones y garantizar que las exenciones (si las hay) estén
  registrado junto con el libro de calibración.