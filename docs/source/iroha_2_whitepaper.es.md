---
lang: es
direction: ltr
source: docs/source/iroha_2_whitepaper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a4e8824c128b9f2a34262a5c9bc09f6b2cd790a0561aa083fa18a987accd7004
source_last_modified: "2026-01-22T15:59:09.647697+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2.0

Hyperledger Iroha v2 es un libro mayor distribuido determinista y tolerante a fallas bizantinas que enfatiza un
Arquitectura modular, valores predeterminados sólidos y API accesibles. La plataforma se envía como un conjunto de cajas Rust.
que pueden integrarse en implementaciones personalizadas o usarse en conjunto para operar una red blockchain de producción.

---

## 1. Descripción general

Iroha 2 continúa la filosofía de diseño introducida con Iroha 1: proporcionar una colección seleccionada de
capacidades listas para usar para que los operadores puedan implementar una red sin escribir grandes cantidades de
código. La versión v2 consolida el entorno de ejecución, el canal de consenso y el modelo de datos en un
espacio de trabajo único y cohesivo.

La línea v2 está diseñada para organizaciones que desean operar sus propias empresas autorizadas o en consorcio.
cadenas de bloques. Cada implementación ejecuta su propia red de consenso, mantiene una gobernanza independiente y puede personalizar
configuración, datos de génesis y cadencia de actualización sin depender de terceros. El espacio de trabajo compartido
permite que se construyan múltiples redes independientes con exactamente la misma base de código mientras se eligen las características y
políticas que coincidan con sus casos de uso.

Tanto Iroha 2 como SORA Nexus (Iroha 3) ejecutan la misma máquina virtual Iroha (IVM). Los desarrolladores pueden crear Kotodama
contratos una vez e implementarlos en redes autohospedadas o en el libro mayor global Nexus sin volver a compilar ni
bifurcando el entorno de ejecución.
### 1.1 Relación con el ecosistema Hyperledger

Los componentes Iroha están diseñados para interoperar con otros proyectos Hyperledger. Consenso, modelo de datos y
Las cajas de serialización se pueden reutilizar en pilas compuestas o junto con implementaciones Fabric, Sawtooth y Besu.
Las herramientas comunes, como los códecs Norito y los manifiestos de gobernanza, ayudan a mantener las interfaces coherentes en todo el mundo.
ecosistema al tiempo que permite que Iroha proporcione una implementación predeterminada obstinada.

### 1.2 Bibliotecas cliente y SDK

Para garantizar experiencias web y móviles de primera clase, el proyecto publica SDK mantenidos:

- `IrohaSwift` para clientes iOS y macOS, integrando aceleración Metal/NEON detrás de respaldos deterministas.
- `iroha_js` para aplicaciones JavaScript y TypeScript, incluidos los constructores Kaigi y los ayudantes Norito.
- `iroha_python` para integraciones de Python, con soporte HTTP, WebSocket y telemetría.
- `iroha_cli` para administración y secuencias de comandos basadas en terminal.

Idiomas y plataformas.

### 1.3 Principios de diseño- **Determinismo primero:** Cada nodo ejecuta las mismas rutas de código y produce los mismos resultados dadas las mismas
  entradas. Las rutas SIMD/CUDA/NEON están controladas por funciones y recurren a implementaciones escalares deterministas.
- **Módulos componibles:** Las redes, el consenso, la ejecución, la telemetría y el almacenamiento se encuentran en espacios dedicados.
  cajas para que los incrustadores puedan adoptar subconjuntos sin tener que cargar toda la pila.
- **Configuración explícita:** Las perillas de comportamiento aparecen a través de `iroha_config`; Los cambios de entorno son
  limitado a las comodidades del desarrollador.
- **Valores predeterminados seguros:** Los códecs canónicos, la aplicación estricta de ABI de puntero y los manifiestos versionados hacen
  actualizaciones entre redes predecibles.

## 2. Arquitectura de plataforma

### 2.1 Composición del nodo

Un nodo Iroha ejecuta varios servicios cooperativos:

- **Torii (`iroha_torii`)** expone las API HTTP/WebSocket para transacciones, consultas, transmisión de eventos y
  telemetría (puntos finales `/v1/...`).
- **Core (`iroha_core`)** coordina la validación, el consenso, la ejecución, la gobernanza y la gestión estatal.
- **Sumeragi (`iroha_core::sumeragi`)** implementa el canal de consenso listo para NPoS con cambios de vista,
  disponibilidad confiable de datos de transmisión y certificados de confirmación. Ver el
  [Guía de consenso Sumeragi](./sumeragi.md) para obtener más detalles.
- **Kura (`iroha_core::kura`)** persiste bloques canónicos, sidecars de recuperación y metadatos de testigos en el disco.
- **World State View (`iroha_core::state`)** almacena la instantánea autorizada en memoria utilizada para la validación
  y consultas.
- **La máquina virtual Iroha (`ivm`)** ejecuta el código de bytes Kotodama (`.to`) y aplica la política ABI del puntero.
- **Norito (`crates/norito`)** proporciona serialización binaria determinista y JSON para cada tipo de cable.
- **Telemetría (`iroha_telemetry`)** exporta métricas Prometheus, registros estructurados y eventos de transmisión.
- **P2P (`iroha_p2p`)** gestiona chismes, topología y conexiones seguras entre pares.

### 2.2 Redes y topología

Los pares Iroha mantienen una topología ordenada derivada del estado comprometido. Cada ronda de consenso selecciona un líder,
conjunto de validación, cola de proxy y validadores del conjunto B. Las transacciones se chismean mediante mensajes codificados con Norito
antes de que el líder los incluya en una propuesta. Garantías de transmisión confiables que bloquean y respaldan
La evidencia llega a todos los pares honestos, asegurando la disponibilidad de datos incluso en condiciones de rotación de la red. Ver cambios rotar
liderazgo cuando se incumplen los plazos, y los certificados de compromiso garantizan que cada bloque comprometido lleve el
conjunto de firmas canónicas utilizado por todos los pares.

### 2.3 Criptografía

La caja `iroha_crypto` potencia la gestión de claves, el hashing y la verificación de firmas:- Ed25519 es el esquema de clave de validación predeterminado.
- Los backends opcionales incluyen Secp256k1, TC26 GOST, BLS (para certificaciones agregadas) y ayudantes ML-DSA.
- Los canales de transmisión combinan identidades Ed25519 con HPKE basado en Kyber para proteger las sesiones de transmisión Norito.
- Todas las rutinas de hash utilizan implementaciones deterministas (SHA-2, SHA-3, Blake2, Poseidon2) con espacio de trabajo
  auditorías documentadas en `docs/source/crypto/dependency_audits.md`.

### 2.4 Puentes de aplicaciones y streaming

- **Transmisión Norito (`iroha_core::streaming`, `norito::streaming`)** proporciona medios cifrados deterministas
  y canales de datos con instantáneas de sesión, rotación de claves HPKE y enlaces de telemetría. Conferencias Kaigi y
  Las transferencias de pruebas confidenciales utilizan esta vía.
- **Connect bridge (`connect_norito_bridge`)** expone una superficie C ABI que impulsa los SDK de plataforma
  (Swift, Kotlin/Android) mientras reutiliza los clientes de Rust bajo el capó.
- **Puente ISO 20022 (`iroha_torii::iso20022_bridge`)** convierte mensajes de pago regulados en Norito
  transacciones, permitiendo la interoperabilidad con los flujos de trabajo financieros sin pasar por alto el consenso o la validación.
- Todos los puentes conservan las cargas útiles Norito deterministas para que los sistemas posteriores puedan verificar las transiciones de estado.

## 3. Modelo de datos

La caja `iroha_data_model` define todos los objetos, instrucciones, consultas y eventos del libro mayor. Aspectos destacados:

- **Domains, accounts, and assets** use canonical I105 account ids and canonical Base58 asset ids. Account aliases are separate on-chain
  bindings in `name@dataspace` / `name@domain.dataspace` form that resolve to I105 account ids, and asset aliases are separate on-chain bindings in `name#dataspace` / `name#domain.dataspace` form that resolve to canonical Base58 asset ids. Metadata is deterministic (`Metadata` map). Numeric assets support fixed-point
  operations; NFTs carry arbitrary structured metadata.

- **Roles y permisos** utilizan tokens enumerados Norito que se asignan directamente a las comprobaciones del ejecutor.
- Los **Disparadores** (basados en tiempo, en bloques o basados en predicados) emiten transacciones deterministas a través de la cadena
  ejecutor.
- **Eventos** se transmiten a través de Torii y reflejan las transiciones de estado comprometidas, incluidos los flujos confidenciales y
  acciones de gobernanza.
- **Las transacciones, bloques y manifiestos** están codificados con Norito (`SignedTransaction`, `SignedBlockWire`) con
  encabezados de versión explícitos, lo que garantiza una decodificación extensible hacia adelante.
- **La personalización** se realiza a través del modelo de datos del ejecutor: los operadores pueden registrar instrucciones personalizadas,
  permisos y parámetros preservando al mismo tiempo el determinismo.
- **Los repositorios (`RepoInstruction`)** permiten agrupar planes de actualización deterministas (ejecutores, manifiestos y
  activos) para que las implementaciones de varios pasos se puedan gestionar en la cadena con la aprobación de la gobernanza.
- **Los artefactos de consenso**, como certificados de confirmación y listas de testigos, residen en el modelo de datos y
  ida y vuelta a través de pruebas doradas para garantizar la compatibilidad entre `iroha_core`, Torii y los SDK.
- **Registros y eventos confidenciales** capturan descriptores de activos protegidos, claves de verificación, compromisos,
  anuladores y cargas útiles de eventos (`ConfidentialEvent::{Shielded,Transferred,Unshielded}`), por lo que los flujos confidenciales
  siguen siendo auditables sin filtrar datos de texto sin formato.

## 4. Ciclo de vida de la transacción1. **Admisión:** Torii decodifica la carga útil Norito, verifica las firmas, el TTL y los límites de tamaño, luego pone en cola el
   transacción localmente.
2. **Chismes:** La transacción se propaga a través de la topología; los pares deduplican mediante hash y repiten la admisión
   cheques.
3. **Selección:** El líder actual extrae transacciones del conjunto pendiente y realiza una validación sin estado.
4. **Simulación con estado:** Las transacciones candidatas se ejecutan dentro de un `StateBlock` transitorio, invocando IVM o
   instrucciones incorporadas. Los conflictos o violaciones de reglas se descartan de manera determinista.
5. **Materialización del activador:** los activadores programados que vencen en la ronda se convierten en transacciones internas.
   y validado utilizando el mismo proceso.
6. **Sellado de propuesta:** Cuando se alcanzan los límites del bloque o expiran los tiempos de espera, el líder emite un código Norito.
   Mensaje `BlockCreated`.
7. **Validación:** Los pares en el conjunto de validación vuelven a ejecutar comprobaciones sin estado/con estado. Signo de compañeros exitosos
   `BlockSigned` mensajes y reenviarlos al conjunto de recopiladores deterministas.
8. **Commit:** Un recopilador reúne un certificado de confirmación una vez que recopila el conjunto de firmas canónicas.
   transmite `BlockCommitted` y finaliza el bloque localmente.
9. **Aplicación:** Todos los pares registran el bloque en Kura, aplican actualizaciones de estado, emiten telemetría/eventos, purgan
   transacciones confirmadas desde el mempool y rotar roles de topología.

Las rutas de recuperación utilizan la transmisión determinista para retransmitir los bloques faltantes y ver los cambios rotan el liderazgo.
cuando vencen los plazos. Los sidecars y la telemetría proporcionan información de diagnóstico sin alterar los resultados del consenso.

## 5. Contratos inteligentes y ejecución

Los contratos inteligentes se ejecutan en la máquina virtual Iroha (IVM):

- **Kotodama** compila fuentes `.ko` de alto nivel en un código de bytes determinista `.to`.
- **La aplicación de puntero ABI** garantiza que los contratos interactúen con la memoria del host a través de tipos de puntero validados.
  Las superficies de llamada al sistema se describen en `ivm/docs/syscalls.md`; la lista ABI tiene hash y versiones.
- **Las llamadas al sistema y los hosts** cubren el acceso al estado del libro mayor, la programación de activadores, las primitivas confidenciales y los medios Kaigi.
  flujos y aleatoriedad determinista.
- **Ejecutor incorporado** continúa admitiendo las Instrucciones Especiales (ISI) Iroha para activos, cuentas, permisos,
  y operaciones de gobernanza. Los ejecutores personalizados pueden ampliar el conjunto de instrucciones respetando los esquemas Norito.
- **Las características confidenciales**, incluidas las transferencias protegidas y los registros de verificación, se exponen a través del ejecutor.
  instrucciones y validadas por anfitriones con compromisos de Poseidón.

## 6. Almacenamiento y persistencia- **Kura block store** escribe cada bloque finalizado como una carga útil `SignedBlockWire` con un encabezado Norito, manteniendo
  encabezados canónicos, transacciones, certificados de confirmación y datos de testigos juntos.
- **World State View** mantiene el estado autorizado en la memoria para consultas rápidas. Instantáneas deterministas y
  Los sidecars de tuberías (`pipeline/sidecars.norito` + `pipeline/sidecars.index`) admiten recuperación y auditorías.
- **La clasificación por niveles** permite la partición activa/fría para implementaciones grandes y al mismo tiempo preserva el carácter determinista.
  validación.
- **Sincronizar y reproducir** cargar bloques comprometidos nuevamente al estado usando las mismas reglas de validación. determinista
  La transmisión garantiza que los pares puedan recuperar los datos faltantes de los vecinos sin depender de un almacenamiento confiable.

## 7. Gobernanza y economía

- Los parámetros en cadena (`SetParameter`) controlan los temporizadores de consenso, los límites de mempool, las perillas de telemetría, las bandas de tarifas,
  y banderas destacadas. Los manifiestos de Génesis generados por `kagami` instalan la configuración inicial.
- Las instrucciones de **Kaigi** administran sesiones colaborativas (crear/unirse/abandonar/finalizar) y alimentar la transmisión Norito.
  Telemetría para casos de uso de conferencias.
- **Hijiri** proporciona reputación determinista de pares y cuentas, integrándose con consenso y admisión.
  políticas y multiplicadores de tarifas (matemáticas de punto fijo Q16). Manifiestos de evidencia, puntos de control y reputación.
  los registros se comprometen en cadena y los perfiles de observadores rigen la procedencia de los recibos.
- **Modo NPoS** (cuando está habilitado) utiliza ventanas electorales respaldadas por VRF y comités ponderados por participación mientras preserva
  valores predeterminados de configuración deterministas.
- **Los registros confidenciales** rigen las claves de verificación de conocimiento cero, los ciclos de vida de las pruebas y los compromisos de
  flujos blindados.

## 8. Experiencia del cliente y herramientas

- **Torii API** ofrece interfaces REST y WebSocket para transacciones, consultas, flujos de eventos, telemetría y
  puntos finales de gobernanza. Las proyecciones JSON se derivan de esquemas Norito.
- **Herramientas CLI** (`iroha_cli`, `iroha_monitor`) cubren la administración, los paneles de control de pares en vivo y la canalización.
  inspección.
- **Herramientas de Génesis** (`kagami`) genera manifiestos codificados con Norito, material de clave de validación y configuración
  plantillas.
- Los **SDK** (Swift, JS/TS, Python) brindan acceso idiomático a instrucciones, consultas, activadores y telemetría.
- **Scripts y enlaces de CI** dentro de `scripts/` automatizan la validación del tablero, la regeneración de códecs y el humo
  pruebas.

## 9. Rendimiento, resiliencia y hoja de ruta- El gasoducto actual apunta a **20 000 tps** con tiempos de bloqueo de **2 a 3 segundos** en condiciones de red favorables.
  condiciones, respaldadas por verificación de firmas por lotes y programación determinista.
- **Telemetría** expone métricas Prometheus para temporizadores de consenso, ocupación de mempool, salud de propagación de bloques,
  Uso de Kaigi y actualizaciones de reputación de Hijiri.
- **Las características de resiliencia** incluyen disponibilidad de datos determinista, complementos de recuperación, rotación de topología y
  Umbrales de visualización/cambio configurables.
- Los hitos futuros de la hoja de ruta (consulte `roadmap.md`) continúan trabajando en los espacios de datos Nexus, confidencialidad mejorada
  herramientas y una aceleración de hardware más amplia, preservando al mismo tiempo los resultados deterministas.

## 10. Operaciones y despliegue

- **Artefactos:** los flujos de trabajo Dockerfiles, Nix flake e `cargo` admiten compilaciones reproducibles. `kagami` emite
  Manifiestos de génesis, claves de validación y configuraciones de ejemplo para implementaciones con permisos y NPoS.
- **Redes autohospedadas:** los operadores administran sus propios conjuntos de pares, reglas de admisión y cadencia de actualización. el
  El espacio de trabajo admite muchas redes Iroha 2 independientes que coexisten sin coordinación y comparten solo el
  código ascendente.
- **Ciclo de vida de configuración:** `iroha_config` resuelve las capas de usuario → real → predeterminadas, asegurando que cada perilla esté
  explícito y controlado por versión. Los cambios en tiempo de ejecución fluyen a través de las instrucciones `SetParameter`.
- **Observabilidad:** `iroha_telemetry` exporta métricas Prometheus, registros estructurados y datos del panel verificados
  mediante scripts CI (`ci/check_swift_dashboards.sh`, `scripts/render_swift_dashboards.sh`,
  `scripts/check_swift_dashboard_data.py`). Los eventos de streaming, consenso y Hijiri están disponibles en
  WebSocket e `scripts/sumeragi_backpressure_log_scraper.py` correlacionan la contrapresión del marcapasos con
  telemetría para la resolución de problemas.
- **Pruebas:** `cargo test --workspace`, pruebas de integración (`integration_tests/`), conjuntos de SDK de idiomas y
  Norito Las luminarias doradas protegen el determinismo. El puntero ABI, las listas de llamadas al sistema y los manifiestos de gobernanza tienen
  pruebas de oro dedicadas.
- **Recuperación:** Los sidecars de Kura, la reproducción determinista y la sincronización de transmisión permiten a los nodos recuperar el estado del disco.
  o compañeros. Los puntos de control de Hijiri y los manifiestos de gobernanza proporcionan instantáneas auditables para el cumplimiento.

# Glosario

Para conocer la terminología a la que se hace referencia en este documento, consulte el glosario de todo el proyecto en
.