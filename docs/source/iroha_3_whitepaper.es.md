---
lang: es
direction: ltr
source: docs/source/iroha_3_whitepaper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 07e149429887b0dfc38cf0619552cbefcbae4dd1ec9fe9e9d47a05371ed08f29
source_last_modified: "2026-01-03T18:07:57.204376+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v3.0 (Vista previa de Nexus)

Este documento captura la arquitectura Hyperledger Iroha v3 de futuro, centrándose en la arquitectura multicarril.
canalización, espacios de datos Nexus y el kit de herramientas de intercambio de activos (AXT). Complementa el documento técnico Iroha v2 de
Describiendo las próximas capacidades que están activamente en desarrollo.

---

## 1. Descripción general

Iroha v3 amplía la base determinista de v2 con escalabilidad horizontal y dominio cruzado más rico
flujos de trabajo. El lanzamiento, con nombre en código **Nexus**, presenta:

- Una red única compartida globalmente llamada **SORA Nexus**. Todos los pares Iroha v3 participan en este universal
  libro mayor en lugar de operar implementaciones aisladas. Las organizaciones se unen registrando sus propios espacios de datos,
  que permanecen aislados por motivos de política y privacidad mientras se anclan en el libro mayor común.
- Una base de código compartida: el mismo repositorio construye tanto Iroha v2 (redes autohospedadas) como Iroha v3 (SORA Nexus).
  La configuración selecciona el modo de destino para que los operadores puedan adoptar las funciones Nexus sin cambiar de software
  pilas. La máquina virtual Iroha (IVM) es idéntica en ambas versiones, por lo que los contratos y el código de bytes Kotodama
  Los artefactos se ejecutan sin problemas en redes autohospedadas y en el libro de contabilidad global Nexus.
- Producción de bloques multicarril para procesar cargas de trabajo independientes en paralelo.
- Espacios de datos (DS) que aíslan los entornos de ejecución sin dejar de ser componibles a través de anclajes en cadena.
- El kit de herramientas de intercambio de activos (AXT) para transferencias de valor atómico a través del espacio e intercambios controlados por contratos.
- Confiabilidad mejorada a través de carriles Reliable Broadcast Commit (RBC), plazos deterministas y pruebas.
  presupuestos muestrales.

Estas características permanecen en desarrollo activo; Las API y los diseños pueden evolucionar antes de la v3 general
hito de disponibilidad. Consulte `nexus.md`, `nexus_transition_notes.md` e `new_pipeline.md` para obtener más información.
detalle a nivel de ingeniería.

## 2. Arquitectura de varios carriles

- **Programador:** Las particiones del programador Nexus funcionan en carriles según los identificadores de espacio de datos y
  grupos de componibilidad. Los carriles se ejecutan en paralelo preservando al mismo tiempo las garantías de orden deterministas dentro
  cada carril.
- **Grupos de carriles:** Los espacios de datos relacionados comparten un `LaneGroupId`, lo que permite la ejecución coordinada de flujos de trabajo que
  abarcan múltiples componentes (por ejemplo, un CBDC DS y su dApp DS de pago).
- **Plazos:** Cada carril realiza un seguimiento de los plazos deterministas (bloqueo, prueba, disponibilidad de datos) para garantizar
  progreso y uso limitado de recursos.
- **Telemetría:** Las métricas a nivel de carril exponen el rendimiento, la profundidad de la cola, las infracciones de los plazos y el uso del ancho de banda.
  Los scripts de CI afirman la presencia de estos contadores para mantener los paneles alineados con el programador.

## 3. Espacios de datos (Nexus)- **Aislamiento:** Cada espacio de datos mantiene su propio carril de consenso, segmento de estado mundial y almacenamiento de Kura. esto
  admite dominios de privacidad y al mismo tiempo mantiene coherente el libro mayor global SORA Nexus a través de anclajes.
- **Anclas:** Las confirmaciones regulares producen artefactos de anclaje que resumen el estado de DS (raíces de Merkle, pruebas,
  compromisos) y publicarlos en el carril global para su auditabilidad.
- **Grupos de carriles y componibilidad:** Los espacios de datos pueden declarar grupos de componibilidad que permitan AXT atómico
  transacciones entre participantes aprobados. La gobernanza controla los cambios de membresía y las épocas de activación.
- **Almacenamiento con código de borrado:** Las instantáneas de Kura y WSV adoptan los parámetros de codificación de borrado `(k, m)` para escalar los datos.
  disponibilidad sin sacrificar el determinismo. Las rutinas de recuperación restauran los fragmentos faltantes de forma determinista.

## 4. Kit de herramientas de intercambio de activos (AXT)

- **Descriptor y enlace:** Los clientes construyen descriptores AXT deterministas. Los anclajes hash `axt_binding`
  descriptores a sobres individuales, evitando la repetición y asegurando que los participantes del consenso validen byte por
  byte de cargas útiles Norito.
- **Llamadas al sistema:** IVM expone las llamadas al sistema `AXT_BEGIN`, `AXT_TOUCH` e `AXT_COMMIT`. Los contratos declaran su
  conjuntos de lectura/escritura por espacio de datos, lo que permite al host imponer la atomicidad en todos los carriles.
- **Identificadores y épocas:** Las billeteras obtienen identificadores de capacidad vinculados a `(dataspace_id, epoch_id, sub_nonce)`.
  Concurrent utiliza conflictos de manera determinista y devuelve códigos canónicos `AxtTrap` cuando se aplican restricciones.
  violado.
- **Aplicación de políticas:** Los hosts principales ahora derivan instantáneas de políticas AXT de los manifiestos del directorio espacial en WSV.
  hacer cumplir las verificaciones de raíz del manifiesto, carril de destino, era de activación, sub-nonce y vencimiento (`current_slot >= expiry_slot`
  aborta) incluso en hosts de prueba mínimos. Las políticas están codificadas por ID de espacio de datos y se crean a partir del catálogo de carriles para que
  los identificadores no pueden escapar de su carril emisor ni utilizar manifiestos obsoletos.
  - Los motivos del rechazo son deterministas: espacio de datos desconocido, falta de coincidencia de la raíz manifiesta, falta de coincidencia del carril de destino,
    handle_era debajo de la activación del manifiesto, sub_nonce debajo del piso de la política, identificador vencido, falta el toque para
    el espacio de datos del identificador o faltan pruebas cuando sea necesario.
- **Pruebas y plazos:** Durante una ventana activa Δ, los validadores recopilan pruebas, muestras de disponibilidad de datos,
  y se manifiesta. El incumplimiento de los plazos cancela el AXT de forma determinista con orientación para los reintentos del cliente.
- **Integración de gobernanza:** Los módulos de políticas definen qué espacios de datos pueden participar en AXT, límite de velocidad
  maneja y publica manifiestos fáciles de usar para auditores que capturan compromisos, anuladores y registros de eventos.

## 5. Líneas de compromiso de transmisión confiable (RBC)- **DA específico del carril:** Los carriles RBC reflejan los grupos de carriles, lo que garantiza que cada tubería de varios carriles tenga datos dedicados
  Garantías de disponibilidad.
- **Presupuestos de muestreo:** Los validadores siguen reglas de muestreo deterministas (`q_in_slot_per_ds`) para validar las pruebas.
  y material testimonial sin coordinación central.
- **Información sobre la contrapresión:** Los eventos del marcapasos Sumeragi se correlacionan con las estadísticas de RBC para diagnosticar carriles bloqueados
  (ver `scripts/sumeragi_backpressure_log_scraper.py`).

## 6. Operaciones y migración

- **Plan de transición:** `nexus_transition_notes.md` describe la migración por fases desde un solo carril (Iroha v2) a
  multicarril (Iroha v3), que incluye preparación de telemetría, activación de configuración y actualizaciones de génesis.
- **Red universal:** Los pares SORA Nexus ejecutan una pila de génesis y gobernanza común. Nuevos operadores a bordo por
  crear un espacio de datos (DS) y satisfacer las políticas de admisión Nexus en lugar de lanzar redes independientes.
- **Configuración:** Los nuevos controles de configuración cubren los presupuestos de carriles, los plazos de prueba, las cuotas de AXT y los metadatos del espacio de datos.
  Los valores predeterminados siguen siendo conservadores hasta que los operadores opten por el modo Nexus.
- **Pruebas:** Las pruebas doradas capturan descriptores de AXT, manifiestos de carril y listas de llamadas al sistema. Pruebas de integración
  (`integration_tests/tests/repo.rs`, `crates/ivm/tests/axt_host_flow.rs`) ejercen flujos de un extremo a otro.
- **Herramientas:** `kagami` obtiene generación de génesis compatible con Nexus y los scripts del panel validan el rendimiento del carril.
  presupuestos de prueba y salud RBC.

## 7. Hoja de ruta

- **Fase 1:** Habilite la ejecución de múltiples carriles en un solo dominio con soporte y auditoría de AXT local.
- **Fase 2:** Activar grupos de componibilidad para AXT entre dominios autorizados y ampliar la cobertura de telemetría.
- **Fase 3:** Implementar la federación completa del espacio de datos Nexus, el almacenamiento con código de borrado y el uso compartido avanzado de pruebas.

Las actualizaciones de estado se encuentran en `roadmap.md` y `status.md`. Las contribuciones que se alinean con el diseño Nexus deben seguir
las políticas deterministas de ejecución y gobernanza establecidas para la v3.