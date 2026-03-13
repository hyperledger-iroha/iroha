---
lang: es
direction: ltr
source: docs/source/global_feature_matrix.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a406b7656a87bb1469444db1cc2d2d5922f16660b53cc7eaef5b838199127e8
source_last_modified: "2026-01-23T20:16:38.056405+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Matriz de características globales

Leyenda: `◉` completamente implementado · `○` mayormente implementado · `▲` parcialmente implementado · `△` implementación recién iniciada · `✖︎` no iniciada

## Consenso y networking

| Característica | Estado | Notas | Evidencia |
|---------|--------|-------|----------|
| Soporte K/r para múltiples coleccionistas y obtención de certificados de primer compromiso | ◉ | Selección determinista del recopilador, distribución redundante, parámetros K/r en cadena y aceptación del primer certificado de confirmación válido enviado con las pruebas. | estado.md:255; estado.md:314 |
| Retroceso del marcapasos, suelo RTT, fluctuación determinista | ◉ | Temporizadores configurables con banda de fluctuación conectada a través de configuración, telemetría y documentos. | estado.md:251 |
| NEW_VIEW selección y seguimiento de control de calidad más alto | ◉ | El flujo de control lleva NEW_VIEW/Evidence, el control de calidad más alto se adopta de forma monótona, el apretón de manos protege la huella digital calculada. | estado.md:210 |
| seguimiento de evidencia de disponibilidad (asesoramiento) | ◉ | Evidencia de disponibilidad emitida y rastreada; commit no determina la disponibilidad en v1. | estado.md:último |
| Transmisión confiable (transporte de carga útil DA) | ◉ | El flujo de mensajes RBC (Init/Chunk/Ready/Deliver) está habilitado cuando `da_enabled=true` como ruta de transporte/recuperación; Se realiza un seguimiento de la evidencia de disponibilidad (asesoramiento) mientras que la confirmación se realiza de forma independiente. | estado.md:último |
| Confirmar enlace estado-raíz de control de calidad | ◉ | Los controles de calidad de confirmación llevan `parent_state_root`/`post_state_root`; no existe una puerta de control de calidad de ejecución separada. | estado.md:último |
| Propagación de evidencia y puntos finales de auditoría | ◉ | Se obtuvieron ControlFlow::Evidence, puntos finales de evidencia Torii y pruebas negativas. | estado.md:176; estado.md:760-761 |
| Telemetría RBC, métricas de preparación/entregadas | ◉ | Puntos finales `/v2/sumeragi/rbc*` y contadores/histograma de telemetría disponibles para los operadores. | estado.md:283-284; estado.md:772 |
| Anuncio de parámetros de consenso y verificación de topología | ◉ | Los nodos transmiten `(collectors_k, redundant_send_r)` y validan la igualdad entre pares. | estado.md:255 |
| Rotación basada en PRF autorizada | ◉ | La selección de líder/recolector autorizado utiliza semilla PRF + altura/vista sobre la lista canónica; La rotación del hash anterior sigue siendo una ayuda heredada. | estado.md:último |

## Oleoducto, Kura y Estado| Característica | Estado | Notas | Evidencia |
|---------|--------|-------|----------|
| Límites de carriles de cuarentena y telemetría | ◉ | Se implementaron controles de configuración, manejo determinista de desbordamiento y contadores de telemetría. | estado.md:263 |
| Perilla de la piscina del trabajador de la tubería | ◉ | `[pipeline].workers` pasó por el inicio de estado con pruebas de análisis de entorno. | estado.md:264 |
| Línea de consulta de instantáneas (cursores almacenados/efímeros) | ◉ | Modo de cursor almacenado con integración Torii y bloqueo de grupos de trabajadores. | estado.md:265; estado.md:371; estado.md:501 |
| Sidecares estáticos de recuperación de huellas dactilares DAG | ◉ | Sidecares almacenados en Kura, validados en el arranque, avisos emitidos en caso de discrepancias. | estado.md:106; estado.md:349 |
| Endurecimiento de la decodificación de hash de la tienda de bloques Kura | ◉ | Las lecturas hash cambiaron al manejo sin formato de 32 bytes con pruebas de ida y vuelta independientes de Norito. | estado.md:608; estado.md:668 |
| Telemetría adaptativa Norito para códecs | ◉ | Métricas de selección de AoS vs NCB agregadas a Norito. | estado.md:156 |
| Consultas instantáneas de WSV a través de Torii | ◉ | El carril de consulta de instantáneas Torii utiliza un grupo de trabajadores de bloqueo y semántica determinista. | estado.md:501 |
| Activar encadenamiento de ejecución de llamadas secundarias | ◉ | Los datos activan la cadena inmediatamente después de la ejecución de la llamada secundaria con orden determinista. | estado.md:668 |

## Norito Serialización y herramientas

| Característica | Estado | Notas | Evidencia |
|---------|--------|-------|----------|
| Norito Migración JSON (espacio de trabajo) | ◉ | Serde retirado de producción; El inventario + barandillas mantienen el espacio de trabajo solo Norito. | estado.md:112; estado.md:124 |
| Lista de denegación de Serde y barreras de protección de CI | ◉ | Los flujos de trabajo/scripts de protección evitan el nuevo uso directo de Serde en todo el espacio de trabajo. | estado.md:218 |
| Norito códec goldens y pruebas AoS/NCB | ◉ | Se agregaron AoS/NCB goldens, pruebas de truncamiento y sincronización de documentos. | estado.md:140-147; estado.md:149-150; estado.md:332; estado.md:666 |
| Herramientas de matriz de características Norito | ◉ | `scripts/run_norito_feature_matrix.sh` admite pruebas de humo posteriores; CI cubre combinaciones de secuencia empaquetada/estructura. | estado.md:146; estado.md:152 |
| Enlaces de lenguaje Norito (Python/Java) | ◉ | Códecs Python y Java Norito mantenidos con scripts de sincronización. | estado.md:74; estado.md:81 |
| Norito Clasificadores estructurales SIMD etapa 1 | ◉ | Clasificadores NEON/AVX2 etapa 1 con arcos cruzados dorados y pruebas de corpus aleatorios. | estado.md:241 |

## Actualizaciones de gobernanza y tiempo de ejecución| Característica | Estado | Notas | Evidencia |
|---------|--------|-------|----------|
| Admisión de actualización en tiempo de ejecución (puerta ABI) | ◉ | Conjunto de ABI activo aplicado en el momento de la admisión con pruebas y errores estructurados. | estado.md:196 |
| Puerta de implementación de espacios de nombres protegidos | ▲ | Implementar requisitos de metadatos y activación por cable; La política/UX sigue evolucionando. | estado.md:171 |
| Puntos finales de lectura de gobierno Torii | ◉ | `/v2/gov/*` leyó las API enrutadas con pruebas de enrutador. | estado.md:212 |
| Eventos y ciclo de vida del registro de claves de verificación | ◉ | Registro/actualización/obsolescencia de VK, eventos, filtros CLI y semántica de retención implementadas. | estado.md:236-239; estado.md:595; estado.md:603 |

## Infraestructura de conocimiento cero

| Característica | Estado | Notas | Evidencia |
|---------|--------|-------|----------|
| API de almacenamiento de archivos adjuntos | ◉ | Puntos finales adjuntos `POST/GET/LIST/DELETE` con identificaciones y pruebas deterministas. | estado.md:231 |
| Trabajador probador de antecedentes e informe TTL | ▲ | Talón del probador detrás de la bandera de característica; TTL GC y perillas de configuración cableados; tubería completa pendiente. | estado.md:212; estado.md:233 |
| Enlace de hash de sobre en CoreHost | ◉ | Verifique los hashes del sobre vinculados a través de CoreHost y expuestos mediante pulsos de auditoría. | estado.md:250 |
| Puerta blindada del historial de raíces | ◉ | Instantáneas de raíz conectadas a CoreHost con historial limitado y configuración de raíz vacía. | estado.md:303 |
| Bloqueos de gobernanza y ejecución de boletas de ZK | ○ | Derivación de anulador, actualizaciones de bloqueo, alternancias de verificación implementadas; El ciclo de vida completo de la prueba aún está madurando. | estado.md:126-128; estado.md:194-195 |
| Adjunto de prueba, verificación previa y desduplicación | ◉ | Los registros de prueba, deduplicación y cordura de las etiquetas de backend persistieron antes de la ejecución. | estado.md:348; estado.md:602 |
| Punto final de búsqueda de prueba ZK Torii | ◉ | `/v2/zk/proof/{backend}/{hash}` expone registros de prueba (estado, altura, vk_ref/compromiso). | estado.md:94 |

## Integración IVM e Kotodama| Característica | Estado | Notas | Evidencia |
|---------|--------|-------|----------|
| Llamada al sistema CoreHost → Puente ISI | ○ | Decodificación de puntero TLV y cola de llamadas al sistema operativas; brechas de cobertura/pruebas de paridad planificadas. | estado.md:299-307; estado.md:477-486 |
| Constructores de punteros y dominios integrados | ◉ | Las funciones integradas Kotodama emiten TLV y SCALL Norito tipificados, con pruebas y documentos IR/e2e. | estado.md:299-301 |
| Validación estricta de Pointer-ABI y sincronización de documentos | ◉ | Política TLV aplicada en todo el host/IVM con pruebas doradas y documentos generados. | estado.md:227; estado.md:317; estado.md:344; estado.md:366; estado.md:527 |
| Puerta de llamada al sistema ZK a través de CoreHost | ◉ | Las colas por operación controlan los sobres verificados y aplican la coincidencia de hash antes de la ejecución de ISI. | cajas/iroha_core/src/smartcontracts/ivm/host.rs:213; cajas/iroha_core/src/smartcontracts/ivm/host.rs:279 |
| Kotodama puntero-Documentos y gramática ABI | ◉ | Gramática/docs sincronizados con constructores en vivo y asignaciones SCALL. | estado.md:299-301 |
| Motor basado en esquemas ISO 20022 y puente Torii | ◉ | Esquemas canónicos ISO 20022 integrados, análisis XML determinista y API `/v2/iso20022/status/{MsgId}` expuesta. | estado.md:65-70 |

## Aceleración de hardware

| Característica | Estado | Notas | Evidencia |
|---------|--------|-------|----------|
| Pruebas de paridad de cola/desalineación SIMD | ◉ | Las pruebas de paridad aleatorias garantizan que las operaciones vectoriales SIMD coincidan con la semántica escalar para una alineación arbitraria. | estado.md:243 |
| Autopruebas y respaldo de metal/CUDA | ◉ | Los backends de GPU ejecutan autopruebas de oro y recurren a escalar/SIMD en caso de discrepancia; las suites de paridad cubren SHA-256/Keccak/AES. | estado.md:244-246 |

## Tiempo de red y modos de consenso

| Característica | Estado | Notas | Evidencia |
|---------|--------|-------|----------|
| Servicio de hora de red (NTS) | ✖︎ | El diseño existe en `new_pipeline.md`; La implementación aún no se ha rastreado en las actualizaciones de estado. | new_pipeline.md |
| Modo de consenso PoS nominado | ✖︎ | Nexus documentos de diseño modos de conjunto cerrado y NPoS; implementación básica pendiente. | new_pipeline.md; nexo.md |

## Nexus Hoja de ruta del libro mayor| Característica | Estado | Notas | Evidencia |
|---------|--------|-------|----------|
| Andamio del contrato del Directorio Espacial | ✖︎ | El contrato de registro global para los manifiestos/gobernanza de DS aún no se ha implementado. | nexo.md |
| Formato de manifiesto y ciclo de vida del espacio de datos | ✖︎ | El esquema de manifiesto Norito, el control de versiones y el flujo de gobernanza permanecen en la hoja de ruta. | nexo.md |
| Gobernanza de DS y rotación de validadores | ✖︎ | Los procedimientos en cadena para la membresía/rotación de DS aún están en fase de diseño. | nexo.md |
| Anclaje Cross-DS y composición de bloques Nexus | ✖︎ | Capa de composición y compromisos de anclaje delineados pero no implementados. | nexo.md |
| Almacenamiento con código de borrado Kura/WSV | ✖︎ | Almacenamiento de blobs/instantáneas con código de borrado para DS público/privado aún no creado. | nexo.md |
| ZK/política de prueba optimista según DS | ✖︎ | Los requisitos de prueba y cumplimiento por DS no se registran en el código. | nexo.md |
| Aislamiento de tarifas/cuotas por espacio de datos | ✖︎ | Las cuotas específicas del DS y los mecanismos de política de tarifas siguen siendo trabajo futuro. | nexo.md |

## Inyección de caos y fallas

| Característica | Estado | Notas | Evidencia |
|---------|--------|-------|----------|
| Izanami Orquestación de red caos | ○ | La carga de trabajo Izanami ahora impulsa la definición de activos, metadatos, NFT y recetas de repetición de activadores con cobertura de unidades para las nuevas rutas. | cajas/izanami/src/instructions.rs; cajas/izanami/src/instructions.rs#tests |