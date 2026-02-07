---
lang: es
direction: ltr
source: docs/source/kaigi_privacy_design.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b7ffca7e960376a2959357cd865d8dab5afa1dfcb959adbc688b6db60977c8f
source_last_modified: "2026-01-04T10:50:53.617088+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Diseño de retransmisión y privacidad de Kaigi

Este documento captura la evolución centrada en la privacidad que introduce el conocimiento cero.
pruebas de participación y relevos estilo cebolla sin sacrificar el determinismo o
auditabilidad del libro mayor.

# Descripción general

El diseño abarca tres capas:

- **Privacidad de la lista**: oculta las identidades de los participantes en la cadena mientras mantiene los permisos del anfitrión y la facturación consistentes.
- **Opacidad de uso**: permite a los hosts registrar el uso medido sin revelar públicamente los detalles por segmento.
- **Retransmisiones superpuestas**: enruta los paquetes de transporte a través de pares de múltiples saltos para que los observadores de la red no puedan saber qué participantes se comunican.

Todas las adiciones siguen siendo Norito-first, se ejecutan bajo ABI versión 1 y deben ejecutarse de manera determinista en hardware heterogéneo.

# Goles

1. Admitir/desalojar a los participantes utilizando pruebas de conocimiento cero para que el libro mayor nunca exponga ID de cuentas sin procesar.
2. Mantener sólidas garantías contables: cada entrada, salida y evento de uso aún deben conciliarse de manera determinista.
3. Proporcionar manifiestos de retransmisión opcionales que describan rutas cebolla para canales de control/datos y que puedan auditarse en cadena.
4. Mantenga operativa la lista alternativa (lista totalmente transparente) para implementaciones que no requieren privacidad.

# Resumen del modelo de amenazas

- **Adversarios:** Observadores de red (ISP), validadores curiosos, operadores de retransmisión maliciosos y hosts semihonestos.
- **Activos protegidos:** Identidad del participante, tiempo de participación, detalles de facturación/uso por segmento y metadatos de enrutamiento de red.
- **Supuestos:** Los anfitriones aún aprenden quién es el verdadero participante fuera de la cadena; los pares del libro mayor verifican las pruebas de manera determinista; los relés superpuestos no son de confianza pero tienen una velocidad limitada; Las primitivas HPKE y SNARK ya existen en el código base.

# Cambios en el modelo de datos

Todos los tipos viven en `iroha_data_model::kaigi`.

```rust
/// Commitment to a participant identity (Poseidon hash of account + domain salt).
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>,
}

/// Nullifier unique to each join action, prevents double-use of proofs.
pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64,
}

/// Relay path description used by clients to set up onion routing.
pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    pub hpke_public_key: FixedBinary<32>,
    pub weight: u8,
}
```

`KaigiRecord` obtiene los siguientes campos:

- `roster_commitments: Vec<KaigiParticipantCommitment>`: reemplaza la lista `participants` expuesta una vez que se habilita el modo de privacidad. Las implementaciones clásicas pueden mantener ambos completos durante la migración.
- `nullifier_log: Vec<KaigiParticipantNullifier>`: estrictamente solo para agregar, limitado por una ventana móvil para mantener los metadatos delimitados.
- `room_policy: KaigiRoomPolicy`: selecciona la postura de autenticación del espectador para la sesión (las salas `Public` reflejan relés de solo lectura; las salas `Authenticated` requieren tickets de espectador antes de que una salida reenvíe paquetes).
- `relay_manifest: Option<KaigiRelayManifest>`: manifiesto estructurado codificado con Norito para que los saltos, las claves HPKE y los pesos permanezcan canónicos sin calzas JSON.
- Enumeración `privacy_mode: KaigiPrivacyMode` (ver más abajo).

```rust
pub enum KaigiPrivacyMode {
    Transparent,
    ZkRosterV1,
}
```

`NewKaigi` recibe campos opcionales coincidentes para que los hosts puedan optar por la privacidad en el momento de la creación.


- Los campos utilizan ayudantes `#[norito(with = "...")]` para aplicar la codificación canónica (little-endian para números enteros, saltos ordenados por posición).
- `KaigiRecord::from_new` siembra los nuevos vectores vacíos y copia cualquier manifiesto de retransmisión proporcionado.

# Cambios en la superficie de instrucciones

## Ayuda de inicio rápido de demostración

Para demostraciones ad hoc y pruebas de interoperabilidad, la CLI ahora expone
`iroha kaigi quickstart`. Él:- Reutiliza la configuración CLI (dominio `wonderland` + cuenta) a menos que se anule mediante `--domain`/`--host`.
- Genera un nombre de llamada basado en marca de tiempo cuando se omite `--call-name` y envía `CreateKaigi` contra el punto final activo Torii.
- Opcionalmente, se une automáticamente al host (`--auto-join-host`) para que los espectadores puedan conectarse inmediatamente.
- Emite un resumen JSON que contiene la URL Torii, identificadores de llamadas, política de privacidad/sala, un comando de unión listo para copiar y la ruta de spool que los probadores deben monitorear (por ejemplo, `storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/*.norito`). Utilice `--summary-out path/to/file.json` para conservar el blob.

Este asistente **no** reemplaza la necesidad de un nodo `irohad --sora` en ejecución: las rutas de privacidad, los archivos de spool y los manifiestos de retransmisión permanecen respaldados por el libro mayor. Simplemente recorta el texto estándar al crear salas temporales para partes externas.

### Script de demostración de un comando

Para una ruta aún más rápida existe un script complementario: `scripts/kaigi_demo.sh`.
Realiza lo siguiente para usted:

1. Firma el `defaults/nexus/genesis.json` incluido en `target/kaigi-demo/genesis.nrt`.
2. Inicia `irohad --sora` con el bloque firmado (registros en `target/kaigi-demo/irohad.log`) y espera a que Torii exponga `http://127.0.0.1:8080/status`.
3. Ejecuta `iroha kaigi quickstart --auto-join-host --summary-out target/kaigi-demo/kaigi_summary.json`.
4. Imprime la ruta al resumen JSON más el directorio de spool (`storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/`) para que pueda compartirlo con evaluadores externos.

Variables de entorno:

- `TORII_URL`: anula el punto final Torii para sondear (predeterminado `http://127.0.0.1:8080`).
- `RUN_DIR`: anula el directorio de trabajo (predeterminado `target/kaigi-demo`).

Detenga la demostración presionando `Ctrl+C`; la trampa en el script finaliza `irohad` automáticamente. Los archivos de spool y el resumen permanecen en el disco para que pueda entregar los artefactos una vez finalizado el proceso.

## `CreateKaigi`

- Valida `privacy_mode` con respecto a los permisos del host.
- Si se proporciona un `relay_manifest`, aplique ≥3 saltos, pesos distintos de cero, presencia de clave HPKE y unicidad para que los manifiestos en cadena sigan siendo auditables.
- Valide la entrada `room_policy` de los SDK/CLI (`public` frente a `authenticated`) y propáguela al aprovisionamiento de SoraNet para que las cachés de retransmisión expongan las categorías GAR correctas (`stream.kaigi.public` frente a `stream.kaigi.authenticated`). Los hosts conectan esto a través de `iroha kaigi create --room-policy …`, el campo `roomPolicy` del JS SDK o configurando `room_policy` cuando los clientes Swift ensamblan la carga útil Norito antes del envío.
- Almacena registros vacíos de compromiso/anulación.

## `JoinKaigi`

Parámetros:

- `proof: ZkProof` (envoltorio de bytes Norito): prueba de Groth16 que certifica que la persona que llama conoce `(account_id, domain_salt)` cuyo hash Poseidon es igual al `commitment` suministrado.
- `commitment: FixedBinary<32>`
- `nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>`: anulación opcional por participante para el siguiente salto.

Pasos de ejecución:

1. Si es `record.privacy_mode == Transparent`, vuelva al comportamiento actual.
2. Verifique la prueba Groth16 con la entrada del registro del circuito `KAIGI_ROSTER_V1`.
3. Asegúrese de que `nullifier` no haya aparecido en `record.nullifier_log`.
4. Adjuntar entradas de compromiso/anulación; si se proporciona `relay_hint`, parchee la vista del manifiesto de retransmisión para este participante (almacenado solo en el estado de sesión en memoria, no en cadena).## `LeaveKaigi`

El modo transparente coincide con la lógica actual.

El modo privado requiere:

1. Prueba de que la persona que llama conoce un compromiso en `record.roster_commitments`.
2. Actualización del anulador que acredite la licencia de un solo uso.
3. Elimine las entradas de compromiso/anulación. La auditoría preserva las lápidas para ventanas de retención fijas para evitar fugas estructurales.

## `RecordKaigiUsage`

Extiende la carga útil con:

- `usage_commitment: FixedBinary<32>`: compromiso con la tupla de uso sin procesar (duración, gas, ID de segmento).
- Prueba ZK opcional que verifica que el delta coincida con los registros cifrados proporcionados fuera del libro mayor.

Los anfitriones aún pueden enviar totales transparentes; El modo de privacidad solo hace que el campo de compromiso sea obligatorio.

# Verificación y circuitos

- `iroha_core::smartcontracts::isi::kaigi::privacy` ahora realiza la lista completa
  verificación por defecto. Resuelve `zk.kaigi_roster_join_vk` (uniones) y
  `zk.kaigi_roster_leave_vk` (sale) de la configuración,
  busca el `VerifyingKeyRef` correspondiente en WSV (asegurándose de que el registro esté
  `Active`, los identificadores de backend/circuito coinciden y los compromisos se alinean), cargos
  contabilidad de bytes y envíos al backend ZK configurado.
- La característica `kaigi_privacy_mocks` conserva el verificador de código auxiliar determinista para que
  Las pruebas unitarias/de integración y los trabajos de CI restringidos se pueden ejecutar sin un backend de Halo2.
  Las compilaciones de producción deben mantener la función desactivada para aplicar pruebas reales.
- La caja emite un error en tiempo de compilación si `kaigi_privacy_mocks` está habilitado en un
  compilación que no es de prueba ni `debug_assertions`, lo que evita la liberación accidental de archivos binarios
  del envío con el talón.
- Los operadores deben (1) registrar la lista de verificadores establecida a través de la gobernanza, y
  (2) configure `zk.kaigi_roster_join_vk`, `zk.kaigi_roster_leave_vk` y
  `zk.kaigi_usage_vk` en `iroha_config` para que los hosts puedan resolverlos en tiempo de ejecución.
  Hasta que las claves estén presentes, las entradas y salidas de privacidad y las llamadas de uso fallan
  deterministamente.
- `crates/kaigi_zk` ahora incluye circuitos Halo2 para unir/abandonar la lista y su uso.
  compromisos junto con los compresores reutilizables (`commitment`, `nullifier`,
  `usage`). Los circuitos de roster exponen la raíz de Merkle (cuatro little-endian
  Miembros de 64 bits) como entradas públicas adicionales para que el anfitrión pueda verificar la prueba.
  contra la raíz de la lista almacenada antes de la verificación. Los compromisos de uso son
  aplicado por `KaigiUsageCommitmentCircuit`, que vincula `(duración, gas,
  segment)` al hash del libro mayor.
- Entradas del circuito `Join`: `(commitment, nullifier, domain_salt)` y privado
  `(account_id)`. Las entradas públicas incluyen `commitment`, `nullifier` y
  cuatro ramas de la raíz de Merkle para el árbol de compromiso de la plantilla (la plantilla
  permanece fuera de la cadena, pero la raíz está vinculada a la transcripción).
- Determinismo: fijamos parámetros de Poseidón, versiones de circuitos e índices en el
  registro. Cualquier cambio pasa de `KaigiPrivacyMode` a `ZkRosterV2` con coincidencia
  pruebas/archivos dorados.

# Superposición de enrutamiento de cebolla

## Registro de retransmisión- Los relés se registran automáticamente como entradas de metadatos de dominio `kaigi_relay::<relay_id>`, incluido el material de clave HPKE y la clase de ancho de banda.
- La instrucción `RegisterKaigiRelay` conserva el descriptor en los metadatos del dominio, emite un resumen `KaigiRelayRegistered` (con huella digital HPKE y clase de ancho de banda) y se puede volver a invocar para rotar claves de manera determinista.
- La gobernanza selecciona las listas permitidas a través de metadatos de dominio (`kaigi_relay_allowlist`) y las actualizaciones de manifiesto/registro de retransmisión exigen la membresía antes de aceptar nuevas rutas.

## Creación manifiesta

- Los hosts construyen rutas de múltiples saltos (longitud mínima 3) a partir de los relés disponibles. El manifiesto codifica la secuencia de AccountIds y las claves públicas HPKE necesarias para cifrar el sobre en capas.
- `relay_manifest` almacenado en cadena contiene descriptores de salto y vencimiento (`KaigiRelayManifest` codificado con Norito); Las claves efímeras reales y las compensaciones por sesión se intercambian fuera del libro mayor mediante HPKE.

## Señalización y medios

- El intercambio SDP/ICE continúa a través de metadatos de Kaigi pero cifrado por salto. Los validadores solo ven el texto cifrado HPKE más los índices de encabezado.
- Los paquetes de medios viajan a través de retransmisiones utilizando QUIC con cargas útiles selladas. Cada salto descifra una capa para conocer la dirección del siguiente salto; El destinatario final obtiene el flujo multimedia después de eliminar todas las capas.

## Conmutación por error

- Los clientes monitorean el estado del relé a través de la instrucción `ReportKaigiRelayHealth`, que persiste los comentarios firmados en los metadatos del dominio (`kaigi_relay_feedback::<relay_id>`), transmite `KaigiRelayHealthUpdated` y permite que los gobiernos/hosts razonen sobre la disponibilidad actual. Cuando falla una retransmisión, el host emite un manifiesto actualizado y registra un evento `KaigiRelayManifestUpdated` (ver más abajo).
- Los hosts aplican cambios de manifiesto en el libro mayor a través de la instrucción `SetKaigiRelayManifest`, que reemplaza la ruta almacenada o la borra por completo. La compensación emite un resumen con `hop_count = 0` para que los operadores puedan observar la transición de regreso al enrutamiento directo.
- Métricas Prometheus (`kaigi_relay_registered_total`, `kaigi_relay_registration_bandwidth_class`, `kaigi_relay_manifest_updates_total`, `kaigi_relay_manifest_hop_count`, `kaigi_relay_health_reports_total`, `kaigi_relay_health_state`, `kaigi_relay_failover_total`, `kaigi_relay_failover_hop_count`) ahora muestra la rotación de retransmisiones, el estado de salud y la cadencia de conmutación por error para los paneles de los operadores.

# Eventos

Ampliar variantes `DomainEvent`:

- `KaigiRosterSummary` – emitido con recuentos anónimos y la lista actual
  root cada vez que cambia la lista (la raíz es `None` en modo transparente).
- `KaigiRelayRegistered`: se emite cada vez que se crea o actualiza un registro de retransmisión.
- `KaigiRelayManifestUpdated`: emitido cuando cambia el manifiesto del relé.
- `KaigiRelayHealthUpdated`: se emite cuando los hosts envían un informe de estado del relé a través de `ReportKaigiRelayHealth`.
- `KaigiUsageSummary`: emitido después de cada segmento de uso, exponiendo solo los totales agregados.

Los eventos se serializan con Norito, exponiendo solo recuentos y hashes de compromiso.Las herramientas CLI (`iroha kaigi …`) envuelven cada ISI para que los operadores puedan registrar sesiones,
envíe actualizaciones de la lista, informe el estado de la retransmisión y registre el uso sin realizar transacciones manualmente.
Los manifiestos de retransmisión y las pruebas de privacidad se cargan desde archivos JSON/hexadecimales pasados.
la ruta de envío normal de la CLI, lo que facilita el contrato de script
admisión en entornos escénicos.

# Contabilidad del gas

- Nuevas constantes en `crates/iroha_core/src/gas.rs`:
  - `BASE_KAIGI_JOIN_ZK`, `BASE_KAIGI_LEAVE_ZK` y `BASE_KAIGI_USAGE_ZK`
    calibrado con respecto a los tiempos de verificación de Halo2 (≈1,6 ms para la lista
    se une/sale, ≈1,2 ms para uso en Apple M2 Ultra). Los recargos continúan
    escalar con tamaño de byte de prueba a través de `PER_KAIGI_PROOF_BYTE`.
- Los compromisos `RecordKaigiUsage` pagan una tarifa adicional según el tamaño del compromiso y la verificación de la prueba.
- El arnés de calibración reutilizará la infraestructura de activos confidenciales con semillas fijas.

# Estrategia de prueba

- Pruebas unitarias que verifican la codificación/decodificación Norito para `KaigiParticipantCommitment`, `KaigiRelayManifest`.
- Pruebas doradas para la vista JSON que garantizan el orden canónico.
- Pruebas de integración para poner en marcha una mini red con (ver
  `crates/iroha_core/tests/kaigi_privacy.rs` para la cobertura actual):
  - Ciclos privados de entrada/salida mediante pruebas simuladas (indicador de función `kaigi_privacy_mocks`).
  - Actualizaciones del manifiesto de retransmisión propagadas a través de eventos de metadatos.
- Pruebas de interfaz de usuario de Trybuild que cubren la configuración incorrecta del host (por ejemplo, falta el manifiesto de retransmisión en modo privado).
- Al ejecutar pruebas unitarias/de integración en entornos restringidos (por ejemplo, el Codex
  sandbox), exporte `NORITO_SKIP_BINDINGS_SYNC=1` para omitir el enlace Norito
  verificación de sincronización aplicada por `crates/norito/build.rs`.

# Plan de Migración

1. ✅ Envíe adiciones al modelo de datos detrás de los valores predeterminados de `KaigiPrivacyMode::Transparent`.
2. ✅ Verificación de doble ruta por cable: la producción deshabilita `kaigi_privacy_mocks`,
   resuelve `zk.kaigi_roster_vk` y ejecuta verificación de sobre real; las pruebas pueden
   aún habilita la función para stubs deterministas.
3. ✅ Se presentó la caja Halo2 dedicada `kaigi_zk`, gas calibrado y cableado
   Cobertura de integración para ejecutar pruebas reales de un extremo a otro (las simulaciones ahora son solo de prueba).
4. ⬜ Desaprobar el vector transparente `participants` una vez que todos los consumidores comprendan los compromisos.

# Preguntas abiertas

- Definir la estrategia de persistencia del árbol Merkle: dentro de la cadena vs fuera de la cadena (inclinación actual: árbol fuera de la cadena con compromisos raíz en la cadena). *(Seguimiento en KPG-201.)*
- Determinar si los manifiestos de retransmisión deben admitir rutas múltiples (rutas redundantes simultáneas). *(Seguimiento en KPG-202.)*
- Aclarar la gobernanza de las reputaciones de los retransmisores: ¿necesitamos recortes o simplemente prohibiciones suaves? *(Seguimiento en KPG-203.)*

Estos elementos deben resolverse antes de habilitar `KaigiPrivacyMode::ZkRosterV1` en producción.