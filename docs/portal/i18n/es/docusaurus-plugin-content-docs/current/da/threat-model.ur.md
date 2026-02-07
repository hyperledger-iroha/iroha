---
lang: es
direction: ltr
source: docs/portal/docs/da/threat-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
ہونے تک دونوں ورژنز کو sincronización رکھیں۔
:::

# Sora Nexus Modelo de amenaza de disponibilidad de datos

_آخری جائزہ: 2026-01-19 -- اگلا شیڈول شدہ جائزہ: 2026-04-19_

Cadencia de mantenimiento: Grupo de Trabajo de Disponibilidad de Datos (<=90 días). ہر ریویژن
`status.md` میں لازمی درج ہو، اور اس میں tickets de mitigación activa اور
artefactos de simulación کے روابط شامل ہوں۔

## مقصد اور دائرہ کار

Disponibilidad de datos (DA) para transmisiones de Taikai, manchas de carril Nexus, gobernanza
artefactos کو Bizantino، نیٹ ورک، اور آپریٹر کی خرابیوں میں بھی قابل بازیافت
رکھتا ہے۔ یہ modelo de amenaza DA-1 (arquitectura اور modelo de amenaza) کیلئے انجینئرنگ
کام کی بنیاد ہے اور tareas DA posteriores (DA-2 تا DA-10) کیلئے baseline ہے۔

Componentes dentro del alcance:
- Extensión de ingesta DA Torii y escritores de metadatos Norito.
- Árboles de almacenamiento de blobs respaldados por SoraFS (niveles activos/inactivos) y políticas de replicación.
- Compromisos de bloque Nexus (formatos de conexión, pruebas, API de cliente ligero).
- Ganchos de aplicación de PDP/PoTR y cargas útiles de DA کیلئے مخصوص ہیں.
- Flujos de trabajo del operador (fijación, desalojo, corte) y canales de observabilidad.
- Aprobaciones de gobernanza جو operadores DA اور contenido کو admitir یا desalojar کرتے ہیں۔Fuera del alcance de este documento:
- Modelado económico مکمل (flujo de trabajo DA-7 میں).
- Protocolos básicos SoraFS y modelo de amenaza SoraFS میں شامل ہیں.
- Ergonomía del SDK del cliente más allá de las consideraciones de superficie de amenaza.

## Descripción general arquitectónica1. **Envío:** Blobs de clientes کو Torii API de ingesta de DA کے ذریعے enviar کرتے ہیں۔
   Los blobs de nodo y los fragmentos de میں بانٹتا ہے، Los manifiestos Norito codifican کرتا ہے (tipo de blob,
   carril, época, indicadores de códec) , trozos de nivel caliente SoraFS میں محفوظ کرتا ہے۔
2. **Anuncio:** Intenciones de PIN y registro de sugerencias de replicación (mercado SoraFS)
   کے ذریعے proveedores de almacenamiento تک جاتے ہیں، etiquetas de políticas کے ساتھ جو retención de calor/frío
   objetivos بتاتے ہیں۔
3. **Compromiso:** Compromisos de blobs de secuenciadores Nexus (CID + raíces KZG opcionales)
   bloque canónico میں شامل کرتے ہیں۔ Hash de compromiso de clientes ligeros اور anunciado
   metadatos کی بنیاد پر disponibilidad verificar کرتے ہیں۔
4. **Replicación:** Nodos de almacenamiento asignados recursos compartidos/fragmentos کھینچتے ہیں، PDP/PoTR
   desafíos مکمل کرتے ہیں، اور política کے مطابق niveles calientes/fríos کے درمیان datos
   promover کرتے ہیں۔
5. **Recuperar:** Consumidores SoraFS یا Puertas de enlace compatibles con DA کے ذریعے recuperación de datos کرتے ہیں،
   pruebas verificar کرتے ہیں، اور réplicas غائب ہونے پر solicitudes de reparación اٹھاتے ہیں۔
6. **Gobernanza:** Parlamento اور Operadores del comité de supervisión de la DA, horarios de alquiler,
   اور intensificaciones de cumplimiento کو aprobar کرتی ہے۔ Artefactos de gobernanza اسی DA
   ruta سے گزرتے ہیں تاکہ transparencia برقرار رہے۔

## Activos y propietariosEscala de impacto: **Crítico** seguridad/vitalidad del libro mayor توڑتا ہے؛ **Alto** Reabastecimiento de DA
یا clientes کو بلاک کرتا ہے؛ Calidad **Moderada** کم کرتا ہے مگر recuperable؛
**Bajo** محدود اثر۔

| Activo | Descripción | Integridad | Disponibilidad | Confidencialidad | Propietario |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (fragmentos + manifiestos) | Taikai, carril, manchas de gobernanza en SoraFS | Crítico | Crítico | Moderado | DA WG / Equipo de almacenamiento |
| Norito Manifiestos DA | Metadatos escritos que describen blobs | Crítico | Alto | Moderado | Grupo de Trabajo sobre el Protocolo Básico |
| Compromisos en bloque | CID + raíces KZG dentro de bloques Nexus | Crítico | Alto | Bajo | Grupo de Trabajo sobre el Protocolo Básico |
| Horarios del PDP/PoTR | Cadencia de aplicación de réplicas de DA | Alto | Alto | Bajo | Equipo de almacenamiento |
| Registro de operadores | Políticas y proveedores de almacenamiento aprobados | Alto | Alto | Bajo | Consejo de Gobierno |
| Registros de alquileres e incentivos | Asientos contables para alquileres y sanciones del DA | Alto | Moderado | Bajo | Grupo de Trabajo de Tesorería |
| Paneles de observabilidad | DA SLO, profundidad de replicación, alertas | Moderado | Alto | Bajo | SRE / Observabilidad |
| Intentos de reparación | Solicitudes para rehidratar los trozos faltantes | Moderado | Moderado | Bajo | Equipo de almacenamiento |

## Adversarios y Capacidades| Actor | Capacidades | Motivaciones | Notas |
| --- | --- | --- | --- |
| Cliente malicioso | Blobs con formato incorrecto envían manifiestos obsoletos, repiten, ingieren, DoS, کوشش۔ | Las transmisiones de Taikai interrumpen la inyección de datos no válidos en کرنا۔ | Claves privilegiadas نہیں۔ |
| Nodo de almacenamiento bizantino | Las réplicas asignadas caen کرنا، PDP/PoTR pruebas falsifican کرنا، colusión کرنا۔ | Retención de DA کم کرنا، alquiler سے بچنا، datos rehenes بنانا۔ | Credenciales de operador válidas رکھتا ہے۔ |
| Secuenciador comprometido | Los compromisos omiten bloques, equivocan, reordenan metadatos | Presentaciones DA چھپانا، inconsistencia پیدا کرنا۔ | Mayoría de consenso سے محدود۔ |
| Operador interno | Abuso de acceso a la gobernanza کرنا، políticas de retención manipulación کرنا، filtración de credenciales کرنا۔ | Beneficio económico, sabotaje۔ | Infraestructura de nivel caliente/frío تک رسائی۔ |
| Adversario de la red | Partición de nodos کرنا، retraso de replicación کرنا، Inyección de tráfico MITM کرنا۔ | Disponibilidad کم کرنا، Los SLO degradan کرنا۔ | TLS permite que los enlaces se ralenticen o caigan |
| Atacante de observabilidad | Paneles/alertas de manipulación de incidentes de کرنا، suprimen کرنا۔ | Cortes de DA چھپانا۔ | Tubería de telemetría تک رسائی درکار۔ |

## Límites de confianza- **Límite de ingreso:** Cliente سے Torii extensión DA ۔ Autenticación a nivel de solicitud, limitación de velocidad,
  اور validación de carga útil درکار ہیں۔
- **Límite de replicación:** Fragmentos de nodos de almacenamiento e intercambio de pruebas کرتے ہیں۔ Nodos
  باہمی autenticado ہیں مگر bizantino برتاؤ ممکن ہے۔
- **Límite del libro mayor:** Bloque de datos comprometidos en almacenamiento fuera de la cadena۔ Consenso
  guardia de integridad کرتا ہے، مگر disponibilidad کیلئے aplicación fuera de la cadena ضروری ہے۔
- **Límite de gobernanza:** Consejo/Parlamento کے فیصلے operadores, presupuestos, recortes
  aprobar کرتے ہیں۔ یہاں خرابی Implementación de DA کو براہ راست متاثر کرتی ہے۔
- **Límite de observabilidad:** Métricas/recopilación de registros, paneles de control/herramientas de alertas
  کو exportar ہونا۔ Interrupciones por manipulación y ataques چھپا سکتا ہے۔

## Escenarios de amenazas y controles

### Ataques de ruta de ingesta

**Escenario:** Cliente malintencionado cargas útiles Norito con formato incorrecto y envío de blobs de gran tamaño
کرتا ہے تاکہ recursos agotados ہوں یا metadatos no válidos شامل ہو۔**Controles**
- Validación del esquema Norito con negociación estricta de la versión; banderas desconocidas rechazan ۔
- Torii punto final de ingesta para limitación de velocidad y autenticación
- SoraFS fragmentador کے ذریعے límites de tamaño de fragmento اور codificación determinista۔
- Tubería de admisión, coincidencia de suma de comprobación de integridad y manifiestos persistentes
- Caché de reproducción determinista (`ReplayCache`) `(lane, epoch, sequence)` seguimiento de Windows
  کرتا ہے، marcas de disco altas پر persist کرتا ہے، اور duplicados/repeticiones obsoletas
  rechazar کرتا ہے؛ propiedad اور fuzz aprovecha huellas dactilares divergentes اور fuera de orden
  las presentaciones cubren کرتے ہیں۔ [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Brechas residuales**
- Torii ingesta کو reproducción caché admisión میں hilo کرنا اور cursores de secuencia
  se reinicia کے پار persist کرنا ضروری ہے۔
- Esquemas DA Norito کے لئے arnés fuzz dedicado (`fuzz/da_ingest_schema.rs`) Más
  ہے؛ paneles de cobertura کو regresión پر alerta کرنا چاہئے۔

### Retención de replicación

**Escenario:** Asignaciones de pines de operadores de almacenamiento bizantinos قبول کرتے ہیں مگر fragmentos





soltar کرتے ہیں، respuestas falsificadas یا colusión سے Los desafíos de PDP/PoTR pasan کرتے ہیں۔**Controles**
- Las cargas útiles de DA del programa de desafío PDP/PoTR extienden ہے اور por cobertura de época دیتا ہے۔
- Replicación de múltiples fuentes con umbrales de quórum؛ buscar fragmentos faltantes del orquestador
  detectar کر کے disparador de reparación کرتا ہے۔
- Gobernanza que reduce las pruebas fallidas y las réplicas faltantes سے vinculadas ہے۔
- Trabajo de conciliación automatizado (`cargo xtask da-commitment-reconcile`) recibos de ingesta
  Compromisos de DA (SignedBlockWire/`.norito`/JSON) para comparar la gobernanza
  El paquete de pruebas JSON emite varios tickets faltantes o no coincidentes que fallan
  ہو کر Alertmanager کو página کرنے دیتا ہے۔

**Brechas residuales**
- Arnés de simulación `integration_tests/src/da/pdp_potr.rs` کا (pruebas:
  `integration_tests/tests/da/pdp_potr_simulation.rs`) colusión y partición
  escenarios چلاتا ہے؛ DA-5 کے ساتھ اسے نئی superficies resistentes کیلئے مزید بڑھائیں۔
- Política de desalojo de nivel frío کیلئے pista de auditoría firmada درکار ہے تاکہ gotas encubiertas
  روکے جا سکیں۔

### Manipulación de compromiso

**Escenario:** Los compromisos de DA del secuenciador comprometido omiten یا alter کرتا ہے، جس سے
fallas de recuperación یا inconsistencias del cliente ligero پیدا ہوتی ہیں۔**Controles**
- Propuestas de bloque de consenso, colas de presentación de DA, verificación cruzada, etc. compañeros
  compromisos faltantes y propuestas rechazadas کرتے ہیں۔
- Las pruebas de inclusión de clientes ligeros verifican کرتے ہیں قبل از buscar identificadores۔
- Recibos de envío, compromisos de bloque y pista de auditoría.
- Trabajo de conciliación automatizado (`cargo xtask da-commitment-reconcile`) recibos de ingesta
  کو compromisos سے comparar کرتا ہے، gobernanza کیلئے JSON evidencia paquete emitir کرتا ہے،
  اور boletos faltantes/no coincidentes پر Página de Alertmanager ہوتا ہے۔

**Brechas residuales**
- Trabajo de conciliación + gancho Alertmanager سے portada؛ paquetes de gobernanza predeterminados میں
  Ingesta de paquete de evidencia JSON کرتے ہیں۔

### Partición de red y censura

**Escenario:** Partición de red de replicación del adversario کرتا ہے، جس سے nodos asignados
trozos حاصل نہیں کر پاتے یا PDP/PoTR desafíos کا جواب نہیں دے پاتے۔

**Controles**
- Los proveedores multirregionales requieren diversas rutas de red یقینی بناتے ہیں۔
- Desafía la fluctuación de Windows y el retroceso del canal de reparación fuera de banda.
- Profundidad de replicación de los paneles de observabilidad, éxito del desafío, latencia de recuperación.
  umbrales de alerta کے ساتھ monitor کرتے ہیں۔

**Brechas residuales**
- Eventos en vivo de Taikai کیلئے simulaciones de partición ابھی نہیں؛ pruebas de remojo ضروری ہیں۔
- Reparar la política de reserva de ancho de banda ابھی codificada نہیں۔

### Abuso interno**Escenario:** El acceso al registro y las políticas de retención del operador manipulan کرتا ہے،
proveedores maliciosos کو lista blanca کرتا ہے، یا alertas suprimen کرتا ہے۔

**Controles**
- Acciones de gobernanza firmas multipartidistas اور Norito registros notariados مانگتی ہیں۔
- Monitoreo de cambios de políticas, registros de archivo y eventos.
- Los registros Norito de canalización de observabilidad de solo agregar con encadenamiento hash aplican کرتا ہے۔
- Manifiesto/reproducción de automatización de revisión de acceso trimestral (`cargo xtask da-privilege-audit`)
  directorios (rutas proporcionadas por el operador) escanean archivos faltantes/sin directorio/escribibles en todo el mundo
  bandera de entradas کرتا ہے، اور paneles de control del paquete JSON firmado کیلئے emite کرتا ہے۔

**Brechas residuales**
- Tablero de pruebas de manipulación کیلئے instantáneas firmadas درکار ہیں۔

## Registro de riesgos residuales| Riesgo | Probabilidad | Impacto | Propietario | Plan de Mitigación |
| --- | --- | --- | --- | --- |
| Caché de secuencia DA-2 سے پہلے Reproducción de manifiestos DA | Posible | Moderado | Grupo de Trabajo sobre el Protocolo Básico | DA-2 میں caché de secuencia + implementación de validación nonce کریں؛ pruebas de regresión شامل کریں۔ |
| >f nodos comprometen la colusión PDP/PoTR | Improbable | Alto | Equipo de almacenamiento | Muestreo entre proveedores کے ساتھ نیا calendario de desafío derivar کریں؛ arnés de simulación سے validar کریں۔ |
| Brecha en la auditoría de desalojos de nivel frío | Posible | Alto | SRE / Equipo de Almacenamiento | Desalojos کیلئے registros de auditoría firmados + recibos en cadena adjuntos کریں؛ tableros de instrumentos سے monitor کریں۔ |
| Latencia de detección de omisiones del secuenciador | Posible | Alto | Grupo de Trabajo sobre el Protocolo Básico | Comparación de recibos nocturnos `cargo xtask da-commitment-reconcile` y compromisos (SignedBlockWire/`.norito`/JSON) کر کے gobernanza کو página کرے۔ |
| Taikai transmite en vivo کیلئے resiliencia de partición | Posible | Crítico | Redes TL | Taladros de partición چلائیں؛ reparar la reserva de ancho de banda کریں؛ documento SOP de conmutación por error کریں۔ |
| Deriva de los privilegios de gobernanza | Improbable | Alto | Consejo de Gobierno | `cargo xtask da-privilege-audit` trimestral (directorios de manifiesto/reproducción + rutas adicionales) con JSON firmado + puerta del tablero; artefactos de auditoría کو ancla en cadena کریں۔ |

## Seguimientos requeridos1. DA ingiere esquemas Norito y vectores de ejemplo publica کریں (DA-2 میں لے جائیں)۔
2. Reproducir caché کو Torii DA ingest میں thread کریں اور se reinician los cursores de secuencia
   کے پار persistir کریں۔
3. **Completado (05/02/2026):** Arnés de simulación PDP/PoTR en colusión + partición
   escenarios اور Ejercicios de modelado de backlog de QoS کرتا ہے؛ دیکھیں
   `integration_tests/src/da/pdp_potr.rs` (pruebas: `integration_tests/tests/da/pdp_potr_simulation.rs`)۔
4. **Completado (29 de mayo de 2026):** `cargo xtask da-commitment-reconcile` recibos de ingesta
   کو compromisos DA (SignedBlockWire/`.norito`/JSON) سے comparar کر کے
   `artifacts/da/commitment_reconciliation.json` emite کرتا ہے اور Alertmanager/
   paquetes de gobernanza کیلئے cableado ہے (`xtask/src/da.rs`)۔
5. **Completado (29 de mayo de 2026):** Carrete de manifiesto/reproducción `cargo xtask da-privilege-audit`
   (rutas proporcionadas por el operador) walk کرتا ہے، desaparecido/no directorio/escribible mundialmente
   bandera de entradas کرتا ہے، اور paneles de gobierno de paquete JSON firmados کیلئے بناتا
   ہے (`artifacts/da/privilege_audit.json`) ۔

**Dónde buscar a continuación:**- DA-2 میں caché de reproducción اور persistencia del cursor آچکی ہے۔ Implementación دیکھیں
  `crates/iroha_core/src/da/replay_cache.rs` (lógica de caché) e integración Torii
  `crates/iroha_torii/src/da/ingest.rs` میں، جو `/v1/da/ingest` کے ذریعے huella digital
  comprueba el hilo کرتا ہے۔
- Arnés de flujo de prueba de simulaciones de transmisión PDP/PoTR کے ذریعے چلتی ہیں:
  `crates/sorafs_car/tests/sorafs_cli.rs`۔ یہ Flujos de solicitudes PoR/PDP/PoTR اور
  los escenarios de falla cubren el modelo de amenaza کرتا ہے جو میں بیان ہیں۔
- Capacidad de remojo de reparación کے نتائج `docs/source/sorafs/reports/sf2c_capacity_soak.md`
  میں ہیں، جبکہ Sumeragi matriz de remojo `docs/source/sumeragi_soak_matrix.md` میں ہے
  (variantes localizadas شامل ہیں)۔ یہ registro de riesgo residual de artefactos کے simulacros
  کو capturar کرتے ہیں۔
- Conciliación + automatización de auditoría de privilegios `docs/automation/da/README.md` اور
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit` Mیں ہے؛
  paquetes de gobernanza کیلئے evidencia adjunta کرتے وقت `artifacts/da/` کی default
  salidas استعمال کریں۔

## Evidencia de simulación y modelado de QoS (2026-02)

Seguimiento DA-1 #3 مکمل کرنے کیلئے ہم نے `integration_tests/src/da/pdp_potr.rs`
(`integration_tests/tests/da/pdp_potr_simulation.rs` سے cubierto) میں


Arnés de simulación determinista PDP/PoTR شامل کیا۔ یہ nodos de arnés کو تین regiones
میں asignar کرتا ہے، probabilidades de hoja de ruta کے مطابق particiones/inyección de colusión
کرتا ہے، Seguimiento de retrasos de PoTR کرتا ہے، اور modelo de acumulación de reparaciones کو feed کرتا ہے جو
Presupuesto de reparación de nivel alto کی عکاسی کرتا ہے۔ Escenario predeterminado (12 épocas, 18 PDP
desafíos + 2 ventanas PoTR por época) سے یہ métricas:<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | Valor | Notas |
| --- | --- | --- |
| Fallos de PDP detectados | 48 / 49 (98,0%) | Particiones اب بھی disparador de detección کرتے ہیں؛ ایک falla no detectada nerviosismo honesto سے ہے۔ |
| Latencia media de detección de PDP | 0,0 épocas | Fallos que originan la época کے اندر ظاہر ہوتے ہیں۔ |
| Fallos PoTR detectados | 28/77 (36,4%) | Detección تب disparador ہوتی ہے جب nodo >=2 ventanas PoTR faltan کرے، زیادہ تر واقعات registro de riesgo residual میں رہتے ہیں۔ |
| Latencia media de detección de PoTR | 2.0 épocas | Escalamiento de archivos میں شامل دو-epoch umbral de retraso سے coincidencia کرتا ہے۔ |
| Pico de cola de reparación | 38 manifiestos | Trabajo pendiente تب بڑھتا ہے جب particiones چار reparaciones/época سے تیز جمع ہوں۔ |
| Latencia de respuesta p95 | 30.068 ms | Ventana de desafío de 30 s اور QoS muestreo کیلئے +/-75 ms jitter کو reflejar کرتا ہے۔ |
<!-- END_DA_SIM_TABLE -->

یہ salidas اب DA tablero prototipos کو conducir کرتے ہیں اور hoja de ruta میں حوالہ
Criterios de aceptación de "arnés de simulación + modelado QoS" پورے کرتے ہیں۔

Automatización según `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
کے پیچھے ہے، جو compartir arnés کو llamada کرتا ہے اور default طور پر Norito JSON
`artifacts/da/threat_model_report.json` میں emite کرتا ہے۔ Trabajos nocturnos یہ archivo
consumir کر کے matrices de documentos actualizar کرتی ہیں اور tasas de detección, reparación
colas, یا muestras de QoS میں deriva پر alerta دیتی ہیں۔Docs کیلئے اوپر کی actualización de tabla کرنے کو `make docs-da-threat-model` چلائیں، جو
`cargo xtask da-threat-model-report` invocar کرتا ہے،
`docs/source/da/_generated/threat_model_report.json` regenerar کرتا ہے، اور
`scripts/docs/render_da_threat_model_tables.py` کے ذریعے یہ reescritura de sección کرتا
ہے۔ `docs/portal` espejo (`docs/portal/docs/da/threat-model.md`) بھی اسی pass میں
actualizar ہوتا ہے تاکہ دونوں copias sincronizar رہیں۔