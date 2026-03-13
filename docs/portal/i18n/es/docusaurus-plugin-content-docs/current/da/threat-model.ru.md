---
lang: es
direction: ltr
source: docs/portal/docs/da/threat-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Канонический источник
Esta página está escrita `docs/source/da/threat_model.md`. Держите обе версии
:::

# Modelo de usuario Disponibilidad de datos Sora Nexus

_Notificación de la fecha: 2026-01-19 — Notificación de la fecha: 2026-04-19_

Частота обслуживания: Grupo de trabajo sobre disponibilidad de datos (<=90 días). Каждая
La redacción de este artículo se puede realizar en `status.md` para obtener entradas activas.
смягчений и артефакты симуляций.

## Цель и область

Программа Data Availability (DA) обеспечивает доступность Transmisión Taikai,
Nexus Manchas de carril y artefactos de gobierno bizantinos, сетевых и операционных
сбоях. Este modelo de cierre del robot DA-1 (arquitecto y modelo)
угроз) и служит базовым ориентиром для дальнейших задач DA (DA-2 .. DA-10).

Componentes de las paredes:
- Расширение Torii DA ingesta y escritores Norito metadatos.
- Blobs de clasificación en SoraFS (niveles fríos/calientes) y replicaciones políticas.
- Compromisos de bloque Nexus (formatos de conexión, pruebas, API de cliente ligero).
- Ganchos para cargas útiles PDP/PoTR para DA.
- Procesos operativos (fijación, desalojo, corte) y tuberías de observabilidad.
- Aprobaciones de gobernanza de для допуска/исключения DA оperatorov и kontentа.Вне рамок документа:
- Modelo económico polaco (instalado en el flujo de trabajo DA-7).
- Los protocolos básicos SoraFS y el modelo de amenaza SoraFS.
- Ergonomía del SDK del cliente muy avanzada.

## Архитектурный обзор

1. **Envío:** Los clientes utilizan blobs para la API de ingesta de DA Torii. Узел
   разбивает blobs, кодирует Norito manifests (тип blob, lane, epoch, flags codec),
   y contiene fragmentos en el nivel activo SoraFS.
2. **Anuncio:** Intenciones de pin y sugerencias de replicación распространяются к
   proveedores de almacenamiento en el registro (mercado SoraFS) con etiquetas de políticas,
   определяющими цели retención de frío/calor.
3. **Compromiso:** Los secuenciadores Nexus incluyen compromisos de blobs (CID + opcional
   Raíces KZG) в канонический блок. Clientes ligeros que trabajan con hash de compromiso y
   объявленную metadatos para comprobar la disponibilidad.
4. **Replicación:** Los nodos de almacenamiento almacenan acciones/fragmentos, проходят
   Los desafíos de PDP/PoTR y los cambios en los niveles políticos calientes/fríos.
5. **Recuperar:** Puede buscar entre SoraFS o puertas de enlace compatibles con DA,
   проверяют pruebas y создают solicitudes de reparación при исчезновении réplicas.
6. **Gobernanza:** Парламент и комитет DA утверждают операторов, horarios de alquiler
   и aplicación de escaladas. Artefactos de gobernanza проходят тем же DA путем для
   прозрачности процесса.## Активы и владельцы

Шкала влияния: **Crítico** ломает безопасность/живучесть libro mayor; **Alto**
блокирует DA backfill или клиентов; **Moderado** снижает качество, но
восстановимо; **Bajo** ограниченное влияние.

| Activo | Descripción | Integridad | Disponibilidad | Confidencialidad | Propietario |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (fragmentos + manifiestos) | Taikai, carril, manchas de gobernanza en SoraFS | Crítico | Crítico | Moderado | DA WG / Equipo de almacenamiento |
| Norito Manifiestos DA | Metadatos tipográficos de blobs | Crítico | Alto | Moderado | Grupo de Trabajo sobre el Protocolo Básico |
| Compromisos en bloque | CID + raíces KZG en bloques Nexus | Crítico | Alto | Bajo | Grupo de Trabajo sobre el Protocolo Básico |
| Horarios del PDP/PoTR | Каденс aplicación de las réplicas DA | Alto | Alto | Bajo | Equipo de almacenamiento |
| Registro de operadores | Proveedores de almacenamiento y políticas nuevos | Alto | Alto | Bajo | Consejo de Gobierno |
| Registros de alquileres e incentivos | Libros de contabilidad para DA, alquiler y fichas | Alto | Moderado | Bajo | Grupo de Trabajo de Tesorería |
| Paneles de observabilidad | DA SLOs, репликации, алерты | Moderado | Alto | Bajo | SRE / Observabilidad |
| Intentos de reparación | Запросы на ре-гидратацию пропавших trozos | Moderado | Moderado | Bajo | Equipo de almacenamiento |

## Противники и возможности| Actor | Возможности | Motivaciones | Примечания |
| --- | --- | --- | --- |
| Cliente malicioso | Eliminación de blobs mal formados, repetición de manifiestos obsoletos, ingesta de DoS. | Transmisión de Срыв Taikai, инъекция невалидных данных. | No utilice llaves privadas. |
| Nodo de almacenamiento bizantino | Dejar caer réplicas, falsificar pruebas de PDP/PoTR, confabularse. | Срезать DA retención, избежать alquiler, удерживать данные. | Имеет валидные credenciales de operador. |
| Secuenciador comprometido | Omitir compromisos, bloquear bloques, reordenar metadatos. | Скрыть envíos de DA, создать несогласованность. | Ограничен консенсусом большинства. |
| Operador interno | Злоупотребление доступом de gobernanza, подмена políticas de retención, утечка credenciales. | Экономическая выгода, саботаж. | Доступ к frío/calor инфраструктуре. |
| Adversario de la red | Partición узлов, задержка replicación, MITM трафик. | Снижение disponibilidad, деградация SLO. | No utilice TLS, ni podrá instalarlo ni enviarlo. |
| Atacante de observabilidad | Подмена paneles/alertas, подавление инцидентов. | Скрыть cortes de DA. | Требует доступа к telemetría canalización. |

## Gran Bretaña- **Límite de ingreso:** Cliente -> Extensión DA Torii. Нужна аутентификация на
  запрос, limitación de velocidad y carga útil de validación.
- **Límite de replicación:** Los nodos de almacenamiento almacenan fragmentos y pruebas. Узлы
  взаимно аутентифицированы, но могут вести себя Bizantino.
- **Límite del libro mayor:** Datos de bloque comprometidos versus almacenamiento fuera de la cadena. consenso
  защищает целостность, но disponibilidad требует aplicación fuera de la cadena.
- **Límites de gobernanza:** Решения Consejo/Parlamento по операторам, бюджетам и
  cortando. Нарушения здесь напрямую влияют на implementación de DA.
- **Límite de observabilidad:** Сбор métricas/registros y экспорт в paneles/alerta
  herramientas. La manipulación provoca interrupciones o ataques.

## Сценарии угроз и контрмеры

### Атаки на ingest путь

**Ejemplo:** El cliente malicioso detecta cargas útiles Norito con formato incorrecto o
blobs de gran tamaño para recursos históricos o metadatos.**Contadores**
- Validación del esquema Norito en todas las versiones anteriores; rechazar banderas desconocidas.
- Limitación de velocidad y autenticación en el punto final de ingesta Torii.
- Tamaño de fragmento personalizado y codificación predeterminada en el fragmento SoraFS.
- La canalización de admisión сохраняет manifiesta только после совпадения checksum.
- Caché de reproducción determinista (`ReplayCache`) отслеживает окна `(lane, epoch, sequence)`,
  eliminar marcas de agua altas en el disco y superar duplicados/repeticiones obsoletas; propiedad
  Y los arneses fuzz eliminan huellas dactilares divergentes y envíos desordenados.
  [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Problemas de seguimiento**
- Torii ingesta para almacenar caché de reproducción en admisión y secuencia de secuencia
  cursores между рестартами.
- Los esquemas DA Norito son un arnés fuzz adicional (`fuzz/da_ingest_schema.rs`) para
  проверки codificar/decodificar инвариантов; paneles de cobertura должны сигнализировать
  при regresión.

### Удержание репликации

**Сценарий:** Los operadores de almacenamiento bizantinos limitan las asignaciones de pines, no eliminan
trozos y проходят PDP/PoTR через respuestas falsificadas o colusión.**Contadores**
- Calendario de desafío PDP/PoTR actualizado en cargas útiles DA según la época.
- Replicación de múltiples fuentes con umbrales de quórum; buscar orquestador выявляет
  fragmentos faltantes y reparación iniciada.
- La reducción de la gobernanza se debe a pruebas fallidas y réplicas faltantes.
- Trabajo de conciliación automatizado (`cargo xtask da-commitment-reconcile`) сравнивает
  Ingerir recibos con compromisos DA (SignedBlockWire/`.norito`/JSON), formato
  Paquete de evidencia JSON para la gobernanza y la solución de tickets faltantes/no coincidentes,
  чтобы Alertmanager puede evitar omisiones/manipulación.

**Problemas de seguimiento**
- Arnés de simulación в `integration_tests/src/da/pdp_potr.rs` (pruebas:
  `integration_tests/tests/da/pdp_potr_simulation.rs`) теперь покрывает colusión
  и partición, проверяя детерминированное выявление bizantino. Продолжать
  расширение вместе с DA-5.
- Политика desalojo de nivel frío требует pista de auditoría firmada, чтобы исключить gotas encubiertas.

### Подмена compromisos

**Ejemplo:** Secuenciador comprometido публикует блоки с пропуском/изменением DA
compromisos, fallas de búsqueda e inconsistencias en el cliente ligero.**Contadores**
- El consenso protege las propuestas de bloque de las colas de envío de DA; compañeros
  отвергают предложения без обязательных compromisos.
- Los clientes ligeros proporcionan pruebas de inclusión antes de los identificadores de búsqueda.
- Registro de auditoría que analiza los recibos de envío y los compromisos de bloque.
- Trabajo de conciliación automatizado (`cargo xtask da-commitment-reconcile`) сравнивает
  ingesta de recibos y compromisos DA (SignedBlockWire/`.norito`/JSON), formulario
  Paquete de evidencia JSON y almohadilla para tickets faltantes/no coincidentes en Alertmanager.

**Problemas de seguimiento**
- Trabajo de conciliación de Закрыто + gancho Alertmanager; paquetes de gobernanza
  по умолчанию ingest-ят paquete de evidencia JSON.

### Partición de red y configuración

**Сценарий:** El adversario разделяет red de replicación, мешая узлам получать
Los fragmentos más populares o los desafíos de PDP/PoTR.

**Contadores**
- Proveedor multirregional требования обеспечивают разнообразные rutas de red.
- Desafía la fluctuación y el retroceso de Windows en la reparación fuera de banda.
- Los paneles de observabilidad monitorean la profundidad de replicación, desafían el éxito y
  recuperar latencia y umbrales de alerta.

**Problemas de seguimiento**
- Simulaciones de partición de eventos en vivo de Taikai пока отсутствуют; нужны pruebas de remojo.
- La reserva política de reparación del ancho de banda no es formalizada.

### Внутреннее злоупотребление**Сценарий:** Operador с доступом к registro манипулирует política de retención,
Lista blanca: proveedores maliciosos o alertas.

**Contadores**
- Las acciones de gobernanza incluyen firmas pluripartidistas y registros notariados Norito.
- Los cambios de política se publican en los registros de seguimiento y archivo.
- Aplicación de canalización de observabilidad: registros Norito de solo agregar y encadenamiento de hash.
- Automatización de revisión de acceso trimestral (`cargo xtask da-privilege-audit`) проходит
  directorios manifiesto/repetición (más información de los operadores), faltante/no directorio/
  entradas de escritura mundial y un paquete JSON firmado para paneles de gobierno.

**Problemas de seguimiento**
- Tablero de pruebas de manipulación con instantáneas firmadas.

## Реестр остаточных рисков| Riesgo | Probabilidad | Impacto | Propietario | Plan de Mitigación |
| --- | --- | --- | --- | --- |
| Reproducir manifiestos DA para el caché de secuencia DA-2 | Posible | Moderado | Grupo de Trabajo sobre el Protocolo Básico | Realización de caché de secuencia + validación nonce en DA-2; добавить pruebas de regresión. |
| Colusión PDP/PoTR por compromisos >f узлов | Improbable | Alto | Equipo de almacenamiento | Вывести новый calendario de desafíos con muestreo entre proveedores; валидировать через arnés de simulación. |
| Brecha en la auditoría de desalojos de nivel frío | Posible | Alto | SRE / Equipo de Almacenamiento | Registrar registros de auditoría firmados y recibos en cadena de desalojos; monitorear los tableros. |
| Secuenciador de omisión de latencia | Posible | Alto | Grupo de Trabajo sobre el Protocolo Básico | Actualmente `cargo xtask da-commitment-reconcile` compara recibos versus compromisos (SignedBlockWire/`.norito`/JSON) y controla los tickets faltantes/no coincidentes. |
| Partición resiliencia для Taikai live streams | Posible | Crítico | Redes TL | Провести taladros de partición; зарезервировать reparación de ancho de banda; documentar la conmutación por error de SOP. |
| Deriva de los privilegios de gobernanza | Improbable | Alto | Consejo de Gobierno | Trimestral `cargo xtask da-privilege-audit` (directorios de manifiesto/reproducción + rutas adicionales) con JSON firmado + puerta del tablero; anclar artefactos de auditoría en la cadena. |

## Seguimientos requeridos1. Publicar esquemas Norito para ingesta de DA y vectores de ejemplo (incluidos en DA-2).
2. Proteger la memoria caché de reproducción con Torii DA ingest y actualizar los cursores de secuencia
   при рестартах узлов.
3. **Terminado (05/02/2026):** Modelo de arnés de simulación PDP/PoTR
   colusión + partición с modelado de backlog de QoS; см. `integration_tests/src/da/pdp_potr.rs`
   (pruebas: `integration_tests/tests/da/pdp_potr_simulation.rs`) с детерминированными сводками.
4. **Completado (29/05/2026):** `cargo xtask da-commitment-reconcile` сравнивает
   ingesta de recibos con compromisos DA (SignedBlockWire/`.norito`/JSON), эмитирует
   `artifacts/da/commitment_reconciliation.json` y soporte para Alertmanager/gobernanza
   paquetes para alertas de omisión/manipulación (`xtask/src/da.rs`).
5. **Completado (2026-05-29):** `cargo xtask da-privilege-audit` проходит manifest/replay
   spool (y rutas de acceso de los operadores), que faltan/no son de directorio/escribibles en todo el mundo y
   Paquete JSON firmado genérico para revisiones de panel/gobernanza
   (`artifacts/da/privilege_audit.json`), elimina la brecha en la automatización de revisión de acceso.

**Para mejorar esto:**- Caché de reproducción y cursores de persistencia aterrizaron en DA-2. Realización en
  `crates/iroha_core/src/da/replay_cache.rs` (lógica de caché) e integración Torii en
  `crates/iroha_torii/src/da/ingest.rs`, el proceso de verificación de huellas dactilares es `/v2/da/ingest`.
- Simulaciones de transmisión PDP/PoTR mejoradas con arnés de flujo de prueba
  `crates/sorafs_car/tests/sorafs_cli.rs`, muestra flujos de solicitud PoR/PDP/PoTR y
  escenarios de falla из модели угроз.
- Capacidad y reparación de remojo результаты в
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, una matriz de inmersión Sumeragi
  `docs/source/sumeragi_soak_matrix.md` (estas variantes locales). Эти
  artefactos фиксируют долгие taladros из реестра рисков.
- Conciliación + automatización de auditoría de privilegios находится в
  `docs/automation/da/README.md` y nuevos comandos
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; используйте
  salidas по умолчанию в `artifacts/da/` при прикреплении evidencia к paquetes de gobernanza.

## Evidencia de simulación y modelado de QoS (2026-02)

Чтобы закрыть DA-1 seguimiento #3, мы реализовали детерминированный PDP/PoTR
arnés de simulación в `integration_tests/src/da/pdp_potr.rs` (pruebas:
`integration_tests/tests/da/pdp_potr_simulation.rs`). Arnés распределяет
узлы по 3 регионам, вводит particiones/colusión согласно вероятностям hoja de ruta,
отслеживает PoTR retraso y питает modelo de acumulación de reparaciones, отражающую hot-tier
presupuesto de reparación. Escenario predeterminado de Запуск (12 épocas, 18 desafíos PDP + 2 PoTR
ventanas por época) дал следующие метрики:<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | Valor | Notas |
| --- | --- | --- |
| Fallos de PDP detectados | 48 / 49 (98,0%) | Particiones все еще детектируются; El dispositivo de detección automático se relaciona con el jitter. |
| Latencia media de detección de PDP | 0,0 épocas | Сбои фиксируются в исходном epoch. |
| Fallos PoTR detectados | 28/77 (36,4%) | Детект срабатывает при пропуске >=2 ventanas PoTR, оставляя большинство событий в registro de riesgo residual. |
| Latencia media de detección de PoTR | 2.0 épocas | Соответствует Umbral de retraso de 2 épocas en escalada de archivos. |
| Pico de cola de reparación | 38 manifiestos | Atraso растет, когда particiones накапливаются быстрее 4 reparaciones/época. |
| Latencia de respuesta p95 | 30.068 ms | Ampliar la ventana de desafío de 30 s con fluctuación de +/-75 ms para el muestreo de QoS. |
<!-- END_DA_SIM_TABLE -->

Estos son los resultados de los prototipos de paneles de control DA y criterios de adaptación
приемки "arnés de simulación + modelado QoS" en la hoja de ruta.

Автоматизация находится за
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
который вызывает общий arnés y эмитит Norito JSON en
`artifacts/da/threat_model_report.json` по умолчанию. Ночные задачи используют
Este archivo de información sobre matrices en documentos y alertas sobre deriva en tasas de detección,
colas de reparación y muestras de QoS.Para las tablas de actualización, utilice `make docs-da-threat-model`, cómo usar `make docs-da-threat-model`
`cargo xtask da-threat-model-report`, пересоздает
`docs/source/da/_generated/threat_model_report.json`, y repite algunas secciones
`scripts/docs/render_da_threat_model_tables.py`. Зеркало `docs/portal`
(`docs/portal/docs/da/threat-model.md`) обновляется в том же проходе для синхронизации.