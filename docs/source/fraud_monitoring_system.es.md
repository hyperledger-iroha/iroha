---
lang: es
direction: ltr
source: docs/source/fraud_monitoring_system.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c8262bacbb15b83bd70c824990e4948832418b59f184bca353eee899e44f4d4
source_last_modified: "2026-01-03T18:07:57.676991+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sistema de monitoreo de fraude

Este documento captura el diseño de referencia para la capacidad compartida de monitoreo de fraude que acompañará al libro mayor central. El objetivo es proporcionar a los proveedores de servicios de pago (PSP) señales de riesgo de alta calidad para cada transacción, manteniendo al mismo tiempo las decisiones de custodia, privacidad y políticas bajo el control de operadores designados fuera del motor de liquidación.

## Metas y criterios de éxito
- Entregue evaluaciones de riesgo de fraude en tiempo real (<120 ms 95p, <40 ms mediana) para cada pago que toque el motor de liquidación.
- Preservar la privacidad del usuario garantizando que el servicio central nunca procese información de identificación personal (PII) y solo ingiera identificadores seudónimos y telemetría conductual.
- Admite entornos multi-PSP donde cada proveedor mantiene autonomía operativa pero puede consultar inteligencia compartida.
- Adaptarse continuamente a nuevos patrones de ataque a través de modelos supervisados ​​y no supervisados ​​sin introducir un comportamiento de libro mayor no determinista.
- Proporcionar seguimientos de decisiones auditables para reguladores y revisores independientes sin exponer carteras o contrapartes sensibles.

## Alcance
- **Dentro del alcance:** Puntuación de riesgo de transacciones, análisis de comportamiento, correlación entre PSP, alertas de anomalías, enlaces de gobernanza y API de integración de PSP.
- **Fuera de alcance:** Aplicación directa (sigue siendo responsabilidad del PSP), evaluación de sanciones (manejada por los canales de cumplimiento existentes) y prueba de identidad (la administración de alias cubre esto).

## Requisitos funcionales
1. **API de puntuación de transacciones**: API síncrona a la que los PSP llaman antes de reenviar un pago al motor de liquidación, devolviendo una puntuación de riesgo, un veredicto categórico y funciones de razonamiento.
2. **Ingestión de eventos**: flujo de resultados de liquidación, eventos del ciclo de vida de la billetera, huellas digitales de dispositivos y comentarios sobre fraude a nivel de PSP para un aprendizaje continuo.
3. **Gestión del ciclo de vida del modelo**: modelos versionados con capacitación fuera de línea, implementación paralela, implementación por etapas y soporte de reversión. Debe existir una heurística alternativa determinista para cada característica.
4. **Bucle de retroalimentación**: los PSP deben poder enviar casos de fraude confirmados, falsos positivos y notas de solución. El sistema alinea los comentarios con las funciones de riesgo y actualiza los análisis.
5. **Controles de privacidad**: Todos los datos almacenados y transmitidos deben estar basados ​​en alias. Cualquier solicitud que contenga metadatos de identidad sin procesar se rechaza y se registra.
6. **Informes de gobernanza**: Exportaciones programadas de métricas agregadas (detecciones por PSP, tipologías, latencia de respuesta) más API de investigación ad hoc para auditores autorizados.
7. **Resiliencia**: Implementación activo-activo en al menos dos instalaciones con drenaje y reproducción automática de colas. Si el servicio se degrada, los PSP recurren a las reglas locales sin bloquear el libro mayor.## Requisitos no funcionales
- **Determinismo y coherencia**: las puntuaciones de riesgo guían las decisiones de PSP pero no modifican la ejecución del libro mayor. Las confirmaciones del libro mayor siguen siendo deterministas entre los nodos.
- **Escalabilidad**: mantenga ≥10 000 evaluaciones de riesgo por segundo con escalamiento horizontal y partición de mensajes codificados por identificadores de pseudomonedero.
- **Observabilidad**: exponer métricas (`fraud.scoring_latency_ms`, `fraud.risk_score_distribution`, `fraud.api_error_rate`, `fraud.model_version_active`) y registros estructurados para cada llamada de puntuación.
- **Seguridad**: TLS mutuo entre los PSP y el servicio central, módulos de seguridad de hardware para firmar sobres de respuesta, pistas de auditoría a prueba de manipulaciones.
- **Cumplimiento**: alinearse con los requisitos ALD/CFT, proporcionar períodos de retención configurables e integrarse con flujos de trabajo de preservación de evidencia.

## Descripción general de la arquitectura
1. **Capa de puerta de enlace API**
  - Recibe solicitudes de puntuación y comentarios a través de API HTTP/JSON autenticadas.
   - Realiza la validación del esquema utilizando códecs Norito y aplica límites de velocidad por identificación de PSP.

2. **Servicio de agregación de funciones**
   - Une las solicitudes entrantes con agregados históricos (velocidad, patrones geoespaciales, uso del dispositivo) almacenados en un almacén de funciones de series temporales.
   - Admite ventanas de funciones configurables (minutos, horas, días) mediante funciones de agregación deterministas.

3. **Motor de riesgo**
   - Ejecuta la canalización del modelo activo (conjunto de árboles potenciados por gradiente, detectores de anomalías, reglas).
   - Incluye un conjunto de reglas deterministas de reserva para garantizar respuestas limitadas cuando las puntuaciones del modelo no están disponibles.
   - Emite sobres `FraudAssessment` con partitura, banda, características contribuyentes y versión del modelo.## Modelos de puntuación y heurísticas
- **Escala de puntuación y bandas**: las puntuaciones de riesgo están normalizadas entre 0 y 1000. Las bandas se definen como: `0–249` (baja), `250–549` (media), `550–749` (alta), `750+` (crítica). Las bandas se asignan a las acciones recomendadas para los PSP (aprobación automática, intensificación, cola para revisión, rechazo automático), pero la aplicación sigue siendo específica de los PSP.
- **Conjunto modelo**:
  - Los árboles de decisión mejorados por gradiente incorporan características estructuradas como cantidad, alias/velocidad del dispositivo, categoría de comerciante, nivel de autenticación, nivel de confianza de PSP y características de gráficos entre billeteras.
  - Un detector de anomalías basado en codificador automático se ejecuta en vectores de comportamiento con ventanas de tiempo (cadencia de gasto por alias, conmutación de dispositivos, entropía temporal). Las puntuaciones se calibran en función de la actividad reciente de PSP para limitar la deriva.
  - Las reglas de política deterministas se ejecutan primero; sus resultados alimentan los modelos estadísticos como características binarias/continuas para que el conjunto pueda aprender interacciones.
- **Heurística alternativa**: cuando falla la ejecución del modelo, la capa determinista aún produce una puntuación limitada al agregar penalizaciones de reglas. Cada regla aporta un peso configurable, que se suma y luego se fija en la escala de 0 a 1000, lo que garantiza la latencia y la explicabilidad en el peor de los casos.
- **Presupuesto de latencia**: puntuación de objetivos de canalización <20 ms para puerta de enlace API + validación, <30 ms para agregación de funciones (servida desde cachés en memoria con escritura retrasada en almacenes persistentes) y <40 ms para evaluación de conjunto. El respaldo determinista regresa dentro de <10 ms si la inferencia de ML excede su presupuesto, lo que garantiza que P95 general se mantenga por debajo de 120 ms.
 - **Presupuesto de latencia**: puntuación de objetivos de canalización <20 ms para puerta de enlace API + validación, <30 ms para agregación de funciones (servida desde cachés en memoria con escritura retrasada en almacenes persistentes) y <40 ms para evaluación de conjunto. El respaldo determinista regresa dentro de <10 ms si la inferencia de ML excede su presupuesto, lo que garantiza que P95 general se mantenga por debajo de 120 ms.## Diseño de caché de funciones en memoria
- **Diseño de fragmentos**: los almacenes de funciones se fragmentan mediante un hash de alias de 64 bits en fragmentos `N = 256`. Cada fragmento posee:
  - Un búfer de anillo sin bloqueo para deltas de transacciones recientes (ventanas de 5 minutos + 1 hora) almacenado como estructura de matrices para maximizar la localidad de la línea de caché.
  - Un árbol Fenwick comprimido (cubos de 16 bits llenos de bits) para mantener agregados las 24 horas, los 7 días de la semana sin un nuevo cálculo completo.
  - Un mapa hash de rayuela que mapea contrapartes → estadísticas continuas (recuento, suma, variación, última marca de tiempo) con un límite de 1024 entradas por alias.
- **Residencia de la memoria**: los fragmentos activos permanecen en la RAM. Para un universo de alias de 50 millones con un 1 % de actividad en la última hora, la residencia de la caché es de aproximadamente 500 000 alias. Con ~320 B por alias de metadatos activos, el conjunto de trabajo es ~160 MB, lo suficientemente pequeño para la caché L3 en servidores modernos.
- **Concurrencia**: los lectores toman prestadas referencias inmutables mediante recuperación basada en épocas; los escritores agregan deltas y actualizan agregados mediante comparar e intercambiar. Esto evita la contención de mutex y mantiene rutas activas hacia dos operaciones atómicas + persecución de punteros acotados.
- **Precargación**: el trabajador de puntuación emite sugerencias `prefetch_read` manuales para el siguiente fragmento de alias una vez que se completa la validación de la solicitud, ocultando la latencia de la memoria principal (~80 ns) detrás de la agregación de funciones.
- **Registro de escritura retrasada**: un WAL por fragmento procesa deltas por lotes cada 50 ms (o 4 KB) y los descarga en el almacén duradero. Los puntos de control se ejecutan cada 5 minutos para mantener estrictos los límites de recuperación.

### Desglose de latencia teórica (servidor Intel Ice Lake, 3,1 GHz)
- **Búsqueda de fragmentos + captación previa**: 1 error de caché (~80 ns) más cálculo de hash (<10 ns).
- **Iteración del buffer circular (32 entradas)**: 32 × 2 cargas = 64 cargas; con 32 líneas de caché B y acceso secuencial, esto permanece en L1 → ~20 ns.
- **Actualizaciones de Fenwick (log₂ 2048 ≈ 11 pasos)**: 11 saltos de puntero; asumiendo que la mitad de L1, la mitad de L2 alcanza → ~30 ns.
- **Sonda de mapa de rayuela (factor de carga 0,75, 2 sondas)**: 2 líneas de caché, ~2 × 15 ns.
- **Ensamblaje de características del modelo**: 150 operaciones escalares (<0,1 ns cada una) → ~15 ns.La suma de estos da ~160 ns de cálculo y ~120 ns de paradas de memoria por solicitud (~0,28 µs). Con cuatro trabajadores de agregación simultáneos por núcleo, la etapa cumple fácilmente el presupuesto de 30 ms incluso bajo carga de ráfaga; La implementación real debe registrar histogramas para validar (a través de `fraud.feature_cache_lookup_ms`).
- **Características de Windows y agregación**:
  - Las ventanas de corto plazo (durante 5 minutos, 1 hora) y de largo plazo (24 horas, 7 días) rastrean la velocidad del gasto, la reutilización del dispositivo y los grados del gráfico de alias.
  - Las funciones de gráficos (por ejemplo, dispositivos compartidos entre alias, distribución repentina, nuevas contrapartes en grupos de alto riesgo) se basan en resúmenes compactados periódicamente para que las consultas permanezcan en menos de un milisegundo.
  - Las heurísticas de ubicación comparan geobuckets aproximados con el comportamiento histórico, señalando saltos improbables (por ejemplo, múltiples ubicaciones distantes en cuestión de minutos) utilizando un incremento de riesgo limitado basado en Haversine.
  - Los detectores de forma de flujo mantienen histogramas continuos de cantidades entrantes/salientes y contrapartes para detectar firmas de mezcla/volteo (entrada rápida seguida de salida similar, secuencias de saltos cíclicos, intermediarios de corta duración).
- **Catálogo de reglas (no exhaustivo)**:
  - **Incumplimiento de velocidad**: series rápidas de transferencias de alto valor que exceden los umbrales por alias o por dispositivo.
  - **Anomalía del gráfico de alias**: Alias ​​interactúa con un grupo vinculado a casos de fraude confirmados o patrones de mulas conocidos.
  - **Reutilización de dispositivos**: huella digital de dispositivo compartida entre alias que pertenecen a diferentes cohortes de usuarios de PSP sin vinculación previa.
  - **Primera vez de alto valor**: Nuevo alias que intenta cantidades superiores al corredor de incorporación típico de PSP.
  - **Rebaja de autenticación**: la transacción utiliza factores más débiles que la línea base de la cuenta (por ejemplo, respaldo de datos biométricos a PIN) sin justificación declarada por el PSP.
  - **Patrón de mezcla/volteo**: Alias ​​participa en cadenas altas de entrada y salida con sincronización estrechamente acoplada, cantidades repetitivas de ida y vuelta o flujos circulares a través de múltiples alias dentro de ventanas cortas. La regla aumenta la puntuación mediante picos de centralidad del gráfico y detectores de forma de flujo; Los casos graves se fijan en la banda `high` incluso antes de la salida ML.
  - **Acierto en la lista negra de transacciones**: el alias o la contraparte aparecen en la lista negra compartida seleccionada mediante votación de gobernanza en cadena o una autoridad delegada con controles `sudo` (por ejemplo, órdenes regulatorias, fraude confirmado). La puntuación se fija en la banda `critical` y emite el código de motivo `BLACKLIST_MATCH`; Los PSP deben registrar las anulaciones para la auditoría.
  - **No coincide la firma del entorno de pruebas**: PSP envía una evaluación generada con una firma de modelo desactualizada; la puntuación aumenta a `critical` y se activa el gancho de auditoría.
- **Códigos de motivo**: cada evaluación incluye códigos de motivo legibles por máquina clasificados por peso de contribución (por ejemplo, `VELOCITY_BREACH`, `NEW_DEVICE`, `GRAPH_HIGH_RISK`, `AUTH_DOWNGRADE`). Los PSP pueden presentarlos a operadores o billeteras para enviar mensajes a los usuarios.- **Gobernanza del modelo**: la calibración y el establecimiento de umbrales siguen manuales documentados: las curvas ROC/PR se revisan trimestralmente, se realizan pruebas retrospectivas contra el fraude etiquetado y los modelos desafiantes se ejecutan en la sombra hasta que se estabilicen. Cualquier actualización de umbral requiere doble aprobación (operaciones de fraude + riesgo independiente).

## Flujo de lista negra basado en la gobernanza
- **Autoría en cadena**: las entradas de la lista negra se introducen a través del subsistema de gobierno (`iroha_core::smartcontracts::isi::governance`) como un ISI `BlacklistProposal` que enumera alias, identificadores de PSP o huellas digitales de dispositivos para bloquear. Las partes interesadas votan utilizando el sistema de votación estándar; una vez que se alcanza el quórum, la cadena emite un registro `GovernanceEvent::BlacklistUpdated` que contiene las adiciones/eliminaciones aprobadas más un `blacklist_epoch` que aumenta monótonamente.
- **Ruta sudo delegada**: las acciones de emergencia se pueden ejecutar mediante la instrucción `sudo::Execute`, que emite el mismo evento `BlacklistUpdated` pero marca el cambio como `origin = Sudo`. Esto refleja la historia en cadena con procedencia explícita para que los auditores puedan distinguir los votos por consenso de las intervenciones delegadas.
- **Canal de distribución**: el servicio de puente FMS se suscribe a la transmisión `LedgerEvent` (codificada con Norito) y busca eventos `BlacklistUpdated`. Cada evento se valida con la prueba de gobernanza Merkle y se verifica con la firma de bloque antes de aplicarse. Los acontecimientos son idempotentes; El FMS mantiene el último `blacklist_epoch` para evitar repeticiones.
- **Aplicación dentro de FMS**: una vez que se acepta una actualización, las entradas se escriben en el almacén de reglas determinista (respaldado por almacenamiento de solo anexos con registros de auditoría). El motor de puntuación recarga en caliente la lista negra en 30 segundos, lo que garantiza que las evaluaciones posteriores activen la regla `BLACKLIST_MATCH` y se fijen en `critical`.
- **Auditoría y reversión**: la gobernanza puede votar para eliminar entradas a través del mismo proceso. El FMS mantiene instantáneas históricas etiquetadas con `blacklist_epoch` para que los operadores puedan responder preguntas forenses o reproducir decisiones pasadas durante las investigaciones.

4. **Plataforma de aprendizaje y análisis**
   - Recibe eventos de fraude confirmados, resultados de acuerdos y comentarios de PSP a través de un libro de contabilidad que solo se adjunta (por ejemplo, Kafka + almacenamiento de objetos).
   - Proporciona cuadernos/trabajos fuera de línea para que los científicos de datos vuelvan a entrenar modelos. Los artefactos modelo se versionan y firman antes de la promoción.

5. **Portal de Gobernanza**
   - Interfaz restringida para que los auditores revisen tendencias, busquen evaluaciones históricas y exporten informes de incidentes.
   - Implementa controles de políticas para que los investigadores no puedan profundizar en la PII sin la cooperación del PSP.

6. **Adaptadores de integración**
   - SDK ligeros para PSP (Rust, Kotlin, Swift, TypeScript) que implementan las solicitudes/respuestas Norito y el almacenamiento en caché local.
   - Gancho del motor de liquidación (dentro de `iroha_core`) que registra referencias de evaluación de riesgos cuando los PSP reenvían transacciones después de la verificación.## Flujo de datos
1. PSP se autentica en la puerta de enlace API y envía un `RiskQuery` que contiene:
   - Identificadores de alias para el pagador/beneficiario, identificación del dispositivo con hash, monto de la transacción, categoría, segmento aproximado de geolocalización, indicadores de confianza de PSP y metadatos de sesiones recientes.
2. Gateway valida la carga útil, la enriquece con metadatos de PSP (nivel de licencia, SLA) y colas para la agregación de funciones.
3. El servicio de funciones extrae los agregados más recientes, construye el vector del modelo y lo envía al motor de riesgos.
4. El motor de riesgos evalúa la solicitud, adjunta códigos de motivo deterministas, firma el `FraudAssessment` y lo devuelve al PSP.
5. PSP combina la evaluación con sus políticas locales para aprobar, rechazar o intensificar la autenticación de la transacción.
6. El resultado (aprobado/rechazado, fraude confirmado/falso positivo) se envía de forma asincrónica a la plataforma de aprendizaje para una mejora continua.
7. Los procesos por lotes diarios acumulan métricas para los informes de gobernanza y envían alertas de políticas (por ejemplo, casos de ingeniería social en aumento) a los paneles de control de PSP.

## Integración con componentes Iroha
- **Core Host Hooks**: la admisión de transacciones ahora aplica los metadatos `fraud_assessment_band` siempre que se configuran `fraud_monitoring.enabled` e `required_minimum_band`. El host rechaza las transacciones que faltan en el campo o que llevan una banda por debajo del mínimo configurado y emite una advertencia determinista cuando `missing_assessment_grace_secs` es distinto de cero (ventana de gracia programada para eliminarse en el hito FM-204 una vez que se conecta el verificador remoto). Las evaluaciones también deben incluir `fraud_assessment_score_bps`; el anfitrión compara la puntuación con la banda declarada (0–249 ➜ baja, 250–549 ➜ media, 550–749 ➜ alta, 750+ ➜ crítica, con valores de puntos básicos admitidos hasta 10000). Cuando se configura `fraud_monitoring.attesters`, las transacciones deben adjuntar un `fraud_assessment_envelope` codificado con Norito (base64) y un `fraud_assessment_digest` (hexadecimal) coincidente. El demonio decodifica de manera determinista el sobre, verifica la firma Ed25519 con el registro de certificación, vuelve a calcular el resumen sobre la carga útil sin firmar y rechaza las discrepancias para que solo las evaluaciones certificadas alcancen un consenso.
- **Configuración**: agregue entradas de configuración en `iroha_config::fraud_monitoring` para los puntos finales del servicio de riesgo, los tiempos de espera y las bandas de evaluación requeridas. Los valores predeterminados desactivan la aplicación de la ley para el desarrollo local.| Clave | Tipo | Predeterminado | Notas |
  | --- | --- | --- | --- |
  | `enabled` | booleano | `false` | Interruptor maestro para controles de admisión; sin `required_minimum_band`, el host registra una advertencia y omite la aplicación de la ley. |
  | `service_endpoints` | matriz | `[]` | Lista ordenada de URL base de servicios antifraude. Los duplicados se eliminan de forma determinista; reservado para el próximo verificador. |
  | `connect_timeout_ms` | duración | `500` | Milisegundos antes de que se cancelen los intentos de conexión; los valores cero se pliegan al valor predeterminado. |
  | `request_timeout_ms` | duración | `1500` | Milisegundos de espera de respuesta del servicio de riesgos. |
  | `missing_assessment_grace_secs` | duración | `0` | Ventana de gracia que permite evaluaciones faltantes; Los valores distintos de cero desencadenan un respaldo determinista que registra y permite la transacción. |
  | `required_minimum_band` | enumeración (`low`, `medium`, `high`, `critical`) | `null` | Cuando se establecen, las transacciones deben adjuntar una evaluación igual o superior a esta banda de gravedad; Se rechazan los valores inferiores. Configúrelo en `null` para deshabilitar la activación incluso si `enabled` es verdadero. |
  | `attesters` | matriz | `[]` | Registro opcional de motores de atestación. Cuando se completan, los sobres deben estar firmados con una de las claves enumeradas e incluir un resumen correspondiente. |

- **Validación**: Las pruebas unitarias en `crates/iroha_core/tests/fraud_monitoring.rs` cubren rutas de banda deshabilitadas, faltantes y de banda insuficiente; `integration_tests::fraud_monitoring_requires_assessment_bands` ejercita el flujo de evaluación simulada de un extremo a otro.

- **Telemetría**: `iroha_telemetry` exporta recopiladores orientados a PSP que capturan recuentos de evaluaciones (`fraud_psp_assessments_total{tenant,band,lane,subnet}`), metadatos faltantes (`fraud_psp_missing_assessment_total{tenant,lane,subnet,cause}`), histogramas de latencia (`fraud_psp_latency_ms{tenant,lane,subnet}`), distribuciones de puntuación (`fraud_psp_score_bps{tenant,band,lane,subnet}`), cargas útiles no válidas. (`fraud_psp_invalid_metadata_total{tenant,field,lane,subnet}`), resultados de certificación (`fraud_psp_attestation_total{tenant,engine,lane,subnet,status}`) y discrepancias de resultados (`fraud_psp_outcome_mismatch_total{tenant,direction,lane,subnet}`). Las claves de metadatos esperadas en cada transacción son `fraud_assessment_band`, `fraud_assessment_tenant`, `fraud_assessment_score_bps`, `fraud_assessment_latency_ms`, el par sobre/resumen del certificador (`fraud_assessment_envelope`, `fraud_assessment_digest`) y el post-incidente. Bandera `fraud_assessment_disposition` (valores: `approved`, `declined`, `manual_review`, `confirmed_fraud`, `false_positive`, `chargeback`, `loss`).
- **Esquema Norito**: defina los tipos Norito para `RiskQuery`, `FraudAssessment` y los informes de gobernanza. Proporcionar pruebas de ida y vuelta para garantizar la estabilidad del códec.

## Privacidad y minimización de datos
- Alias, ID de dispositivos con hash y depósitos de geolocalización aproximados forman todo el plano de datos compartido con el servicio central.
- los PSP conservan la correspondencia entre alias y identidades reales; ningún mapeo de este tipo sale de su perímetro.
- Los modelos de riesgo operan solo con señales de comportamiento seudónimas más el contexto enviado por PSP (categoría de comerciante, canal, nivel de autenticación).
- Las exportaciones de auditoría se agregan (por ejemplo, recuentos por PSP por día). Cualquier análisis requiere control dual y anonimización por parte de PSP.## Operaciones e implementación
- Implementar la plataforma de puntuación como un subsistema dedicado gestionado por un operador designado distinto de los operadores de nodos del banco central.
- Proporcionar entornos azul/verde: `fraud-scoring-prod`, `fraud-scoring-shadow`, `fraud-lab`.
- Implementar controles de estado automatizados (latencia de API, acumulación de mensajes, éxito de carga del modelo). Si las comprobaciones de estado fallan, los SDK de PSP cambian automáticamente al modo solo local y notifican a los operadores.
- Mantener depósitos de retención: almacenamiento en caliente (30 días en la tienda de funciones), almacenamiento en caliente (1 año en almacenamiento de objetos), archivo en frío (5 años comprimidos).

## Paneles y recopiladores de telemetría

### Recolectores requeridos

- **Prometheus scrape**: habilite `/metrics` en cada validador que ejecute el perfil de integración de PSP para que se exporten las series `fraud_psp_*`. Las etiquetas predeterminadas incluyen los ID de marcador de posición `subnet="global"` e `lane` para que los paneles puedan girar una vez que se envía el enrutamiento de múltiples subredes.
- **Totales de evaluaciones**: `fraud_psp_assessments_total{tenant,band}` cuenta las evaluaciones aceptadas por banda de gravedad; alerta de incendio si un inquilino deja de informar durante 5 minutos.
- **Metadatos faltantes**: `fraud_psp_missing_assessment_total{tenant,cause}` distingue los rechazos firmes (`cause="missing"`) de las asignaciones de la ventana de gracia (`cause="grace"`). Transacciones de puerta que caen repetidamente en el grupo de gracia.
- **Histograma de latencia**: `fraud_psp_latency_ms_bucket` rastrea la latencia de puntuación informada por PSP. Objetivo 20% de la media final de 30 días.
- **Metadatos no válidos**: `fraud_psp_invalid_metadata_total{field}` marca las regresiones de la carga útil de PSP (por ejemplo, ID de inquilinos faltantes, disposiciones con formato incorrecto) para que las actualizaciones del SDK se puedan implementar rápidamente.
- **Estado de la atestación**: `fraud_psp_attestation_total{tenant,engine,status}` confirma que los sobres se están firmando y los resúmenes coinciden. Alerta si `status!="verified"` aumenta para cualquier inquilino o motor.

### Cobertura del panel

- **Resumen ejecutivo**: gráfico de áreas apiladas de `fraud_psp_assessments_total` por banda por inquilino, junto con una tabla que resume la latencia P95 y los recuentos de discrepancias.
- **Operaciones**: paneles de histograma para `fraud_psp_latency_ms` e `fraud_psp_score_bps` con comparación semana tras semana, además de contadores de estadística única para `fraud_psp_missing_assessment_total` divididos por `cause`.
- **Monitoreo de riesgos**: gráfico de barras de `fraud_psp_outcome_mismatch_total` por inquilino, tabla desglosada que enumera los casos recientes de `fraud_assessment_disposition=confirmed_fraud` donde `band` era `low` o `medium`.
- **Reglas de alerta**:
  - `rate(fraud_psp_missing_assessment_total{cause="missing"}[5m]) > 0` → alerta de paginación (admisión rechazando tráfico PSP).
  - `histogram_quantile(0.95, sum(rate(fraud_psp_latency_ms_bucket[10m])) by (le,tenant)) > 150` → incumplimiento de SLO de latencia.
  - `sum by (tenant) (rate(fraud_psp_outcome_mismatch_total{direction="missed_fraud"}[1h])) > 0.01` → deriva del modelo/brecha de políticas.

### Expectativas de conmutación por error- Los SDK de PSP deben mantener dos puntos finales de puntuación activos y realizar una conmutación por error dentro de los 15 segundos posteriores a la detección de errores de transporte o picos de latencia >200 ms. El libro mayor tolera el tráfico de gracia durante como máximo `fraud_monitoring.missing_assessment_grace_secs`; Los operadores deben mantener la perilla en <= 30 segundos en producción.
- Los validadores registran `fraud_psp_missing_assessment_total{cause="grace"}` mientras están en reserva; Si un inquilino permanece en gracia durante más de 5 minutos, el PSP debe cambiar a revisión manual y abrir un incidente Sev2 con el equipo de operaciones de fraude compartido.
- Las implementaciones activo-activo deben demostrar drenaje/reproducción de cola durante los simulacros de recuperación ante desastres. Las métricas de reproducción deben mantener `fraud_psp_latency_ms` P99 por debajo de 400 ms para la ventana de reproducción.

## Lista de verificación para compartir datos de PSP

1. **Plomería de telemetría**: exponer las claves de metadatos enumeradas anteriormente para cada transacción entregada al libro mayor; Los identificadores de inquilinos deben ser seudónimos y estar sujetos al contrato de PSP.
2. **Anonimización**: confirme que los hashes del dispositivo, los identificadores de alias y las disposiciones estén seudonimizados antes de abandonar el perímetro de PSP; no se puede incrustar ninguna PII en los metadatos Norito.
3. **Informes de latencia**: complete `fraud_assessment_latency_ms` con temporización de extremo a extremo (puerta de enlace a PSP) para que las regresiones de SLA aparezcan de inmediato.
4. **Conciliación de resultados**: actualice `fraud_assessment_disposition` una vez que se confirmen los casos de fraude (por ejemplo, se publique una devolución de cargo) para mantener precisas las métricas de discrepancia.
5. **Simulacros de conmutación por error**: ensaye trimestralmente utilizando la lista de verificación compartida: verifique la conmutación por error automática del punto final, garantice el registro de la ventana de gracia y adjunte notas de exploración a la tarea de seguimiento presentada por `scripts/ci/schedule_fraud_scoring.sh`.
6. **Validación del panel**: los equipos de operaciones de PSP deben revisar los paneles Prometheus después de la incorporación y después de cada ejercicio del equipo rojo para confirmar que las métricas fluyen con las etiquetas de inquilinos esperadas.

## Consideraciones de seguridad
- Todas las respuestas están firmadas con claves respaldadas por hardware; Los PSP validan las firmas antes de confiar en las puntuaciones.
- Límite de velocidad por alias/dispositivo para mitigar los ataques de sondeo destinados a conocer los límites del modelo.
- Incrustar marcas de agua dentro de las evaluaciones para rastrear las respuestas filtradas sin revelar públicamente la identidad del PSP.
- Realizar ejercicios trimestrales del equipo rojo en coordinación con el Grupo de Trabajo de Seguridad (Milestone 0) e incorporar los hallazgos a las actualizaciones de la hoja de ruta.## Fases de implementación
1. **Fase 0 – Cimentaciones**
   - Finalizar los esquemas Norito, el andamiaje del SDK de PSP, el cableado de configuración y el talón de verificación del lado del libro mayor.
   - Construir un motor de reglas determinista que cubra comprobaciones de riesgo obligatorias (velocidad, velocidad por par de alias, reutilización de dispositivos).
2. **Fase 1 – MVP de puntuación central**
   - Implementar tienda de funciones, servicio de puntuación y paneles de telemetría.
   - Integrar puntuación en tiempo real con una cohorte limitada de PSP; capturar métricas de latencia y calidad.
3. **Fase 2: Análisis avanzado**
   - Introducir detección de anomalías, análisis de enlaces basado en gráficos y umbrales adaptativos.
   - Lanzar el portal de gobernanza y los canales de informes por lotes.
4. **Fase 3: Aprendizaje continuo y automatización**
   - Automatice los canales de capacitación/validación de modelos, agregue implementaciones canarias y amplíe la cobertura del SDK.
   - Alinearse con acuerdos de intercambio de datos entre jurisdicciones y conectarse a futuros puentes de subredes múltiples.

## Preguntas abiertas
- ¿Qué organismo regulador constituirá el operador del servicio de fraude y cómo se comparten las responsabilidades de supervisión?
- ¿Cómo exponen los PSP los flujos de desafíos de los usuarios finales manteniendo al mismo tiempo una experiencia de usuario coherente entre los proveedores?
- ¿Qué tecnologías que mejoran la privacidad (por ejemplo, enclaves seguros, agregación homomórfica) deberían priorizarse una vez que el servicio básico sea estable?