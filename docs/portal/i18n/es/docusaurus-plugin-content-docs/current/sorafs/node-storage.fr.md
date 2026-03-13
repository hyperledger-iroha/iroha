---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-storage.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: almacenamiento de nodo
título: Conception du stockage du nœud SoraFS
sidebar_label: Concepción del stockage du nœud
descripción: Arquitectura de almacenamiento, cuotas y ganchos de ciclo de vida para los nuevos Torii hébergeant des données SoraFS.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/sorafs_node_storage.md`. Guarde las dos copias sincronizadas justo al retrato de la documentación histórica de la Esfinge.
:::

## Conception du stockage du nœud SoraFS (Brouillon)

Esta nota precisa comentario un nœud Iroha (Torii) puede inscribirse en el sofá
disponibilidad de données SoraFS y reserva una parte del disco local para
stocker et servir des chunks. Elle completa la especificación de descubrimiento.
`sorafs_node_client_protocol.md` y el trabajo de los accesorios SF-1b en detalle
la arquitectura de la zona de almacenamiento, los controles de recursos y la fontanería de
configuración que debe llegar en los caminos de código del nœud y del gateway.
Los operadores de taladros se encuentran en el
[Runbook d'opérations du nœud](./node-operations).

### Objetivos- Permitir toda validación o proceso Iroha auxiliar de exposición del disco
  Disponible en tanto que proveedor SoraFS sin afectar las responsabilidades del libro mayor.
- Garder le module de stockage déterministe et piloté par Norito: manifiestos,
  Planes de fragmentos, pruebas de recuperación de datos (PoR) y anuncios de proveedores.
  sont la source de verité.
- Aplicación de cuotas definidas por el operador según lo que un nuevo usuario haya dejado de pasar.
  recursos propios en aceptar trop de demandes de pin ou de fetch.
- Exposer la santé/la télémétrie (échantillonnage PoR, latencia de recuperación de fragmentos,
  pression disque) à la gouvernance et aux client.

### Arquitectura de alto nivel

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Módulos clés :- **Puerta de enlace**: expone los puntos finales HTTP Norito para las propuestas de pin,
  les requêtes de fetch de chunks, l'échantillonnage PoR et la télémétrie. yo
  Valide las cargas útiles Norito y solicite las solicitudes para el almacén de fragmentos. Reutilizar
  La pila HTTP Torii existe para eliminar un nuevo demonio.
- **Registro de PIN**: el estado de los pines de manifiesto sigue en `iroha_data_model::sorafs`
  y `iroha_core`. Lorsqu'un manifest est Accepté, le registre registre le digest
  du manifest, le digest du plan de chunk, la racine PoR et les flags de capacité du
  proveedor.
- **Almacenamiento de fragmentos**: implementación `ChunkStore` en disco que contiene manifiestos
  signés, materialice los planes de fragmentación a través de `ChunkProfile::DEFAULT` y persista
  Los fragmentos se ajustan a un diseño determinado. Cada trozo está asociado a una huella digital
  de contenu et à des métadonnées PoR afin que l'échantillonnage puisse revalider
  sin recuperar el archivo completo.
- **Cuota/Programador**: impone los límites configurados por el operador (bytes de disco máximo,
  pines atentos al máximo, búsquedas paralelas máximas, TTL de fragmentos) y coordinación de IO para que
  les tâches ledger ne soient pas affamées. El planificador es igualmente responsable de
  service des preuves PoR et des requêtes d'échantillonnage avec CPU borné.

### Configuración

Agregue una nueva sección a `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # tag optionnel lisible
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```- `enabled`: alternar participación. Cuando es falso, le gateway renvoie 503 pour les
  Los puntos finales de almacenamiento y el número no se anuncian en descubrimiento.
- `data_dir` : repertorio racine des données de chunk, árboles PoR y télémétrie de
  buscar. Por defecto `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: límite de duración para los trozos de pino. Un tache de fond
  Rejette les nouveaux pins quand la limite est atteinte.
- `max_parallel_fetches`: panel de concurrencia impuesto por el programador para
  equilibrer bande passante/IO disco con la carga del validador.
- `max_pins`: nombre máximo de pines de manifiesto que el nuevo aceptado antes de aplicar
  desalojo/contrapresión.
- `por_sample_interval_secs`: cadencia de trabajos de échantillonnage PoR automáticos. trabajo chaque
  échantillonne `N` feuilles (configurable por manifiesto) y émet des événements de télémétrie.
  La gobernanza puede ser escaladora `N` de manera determinada a través de la clé de métadonnées
  `profile.sample_multiplier` (entrada `1-4`). El valor puede ser un nombre/una cadena única.
  O un objeto con anulaciones de perfil, por ejemplo `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estructura utilizada por el generador de anuncios para replicar los campeones.
  `ProviderAdvertV1` (puntero de participación, sugerencias de QoS, temas). Si omis, le nœud utiliza les
  defaults du registre de gouvernance.

Plombería de configuración:- `[sorafs.storage]` está definido en `iroha_config` como `SorafsStorage` y cargado
  Después del archivo de configuración del nuevo.
- `iroha_core` e `iroha_torii` transfieren la configuración de almacenamiento a la puerta de enlace del constructor
  et au chunk store au démarrage.
- Des anula dev/test existente (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), más
  Las implementaciones de producción deben agregarse al archivo de configuración.

### CLI de utilidades

Mientras la superficie HTTP Torii está en curso de cableado, la caja
`sorafs_node` embarque un CLI fino para que los operadores puedan scripter des
Ejercicios de ingesta/exportación frente al backend persistente.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` asiste a un manifiesto `.to` codificado Norito y los bytes de carga útil asociados.
  Il reconstruit le plan de fragment después de le profil de fragmentación du manifest, imponer
  la parité des digests, persiste les fichiers de chunk, y émet en option un blob
  JSON `chunk_fetch_specs` para que las herramientas posteriores puedan validar el diseño.
- `export` acepta un ID de manifiesto y escribe el manifiesto/carga útil almacenado en disco
  (con plan JSON opcional) para que los accesorios sean reproducibles.Los dos comandos imprimen un currículum Norito JSON en la salida estándar, lo que facilita el
tuberías en los scripts. La CLI está cubierta por una prueba de integración para garantizar
Los manifiestos y cargas útiles se corrigen de ida y vuelta antes de la llegada de las API Torii.【crates/sorafs_node/tests/cli.rs:1】

> Paridad HTTP
>
> Le gateway Torii expone el desorden de los ayudantes en una conferencia solo básica sobre el mismo
> `NodeHandle` :
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — enviar el manifiesto
> Norito almacenado (base64) con digest/métadonnées.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — enviar el plan de fragmento
> Determina JSON (`chunk_fetch_specs`) para las herramientas posteriores.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Estos puntos finales reflejan la salida CLI para que las tuberías puedan pasar
> des scripts locaux aux probes HTTP sin cambiador de analizadores.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Ciclo de vida del nuevo1. **Démarrage** :
   - Si el almacenamiento está activado, el nuevo inicializará el almacén de trozos con el repertorio.
     y la capacidad configurada. Cela incluye la verificación o la creación de la base.
     de données de manifest PoR et le replay des manifests pinés pour chauffer les caches.
   - Registrar las rutas de la puerta de enlace SoraFS (puntos finales Norito JSON POST/GET para pin,
     buscar, échantillonnage PoR, télémétrie).
   - Lanzar el trabajador de échantillonnage PoR y el monitor de cuotas.
2. **Descubrimiento/Anuncios**:
   - Generador de documentos `ProviderAdvertV1` con capacidad/santé courante, les signer
     avec la clé approuvée par le conseil, et publier via le canal de discovering.
     Utilice la nueva lista `profile_aliases` para guardar las manijas canónicas
3. **Flujo de trabajo de pin**:
   - Le gateway recibe un manifiesto firmado (incluye plan de fragmentación, racine PoR, firmas
     del consejo). Valide la lista de alias (`sorafs.sf1@1.0.0` requisitos) y asegure que
     El plan de fragmentación corresponde a los metadonnées del manifiesto.
   - Verificador de cuotas. Si los límites de capacidad/pins serán superados, responderá
     por un error político (estructura Norito).
   - Transmita los trozos donados en `ChunkStore` para verificar los digestiones durante la ingestión.
     Mettre à jour les arbres PoR et stocker les métadonnées du manifest dans le registre.
4. **Flujo de trabajo de recuperación**:- Servir las solicitudes de rango de trozos después del disco. El planificador impone
     `max_parallel_fetches` y envíe `429` en caso de saturación.
   - Emettre une télémétrie structurée (Norito JSON) con latencia, bytes de servicio y
     Computadores de errores para el monitoreo posterior.
5. **Échantillonnage PoR** :
   - El trabajador selecciona los manifiestos proporcionales a los pesos (por ejemplo, bytes almacenados)
     y ejecute un échantillonnage determinado a través del árbol PoR du chunk store.
   - Conservar los resultados de las auditorías de gobierno e incluir los currículums en
     proveedor de anuncios / puntos finales de televisión.
6. **Desalojo / aplicación de cuotas** :
   - Quand la capacidad est atteinte, le nœud rejette les nouveaux pins por defecto. es
     Opción, los operadores pueden configurar las políticas de desalojo (ej. TTL, LRU)
     une fois le modèle de gouvernance défini ; Para el instante, el diseño supone des
     cuotas estrictas y des operaciones de desconexión iniciadas por el operador.

### Declaración de capacidad e integración de la programación- Torii relé desormado las actualizaciones del día `CapacityDeclarationRecord` después de `/v2/sorafs/capacity/declare`
  vers le `CapacityManager` embarqué, de sorte que cada nœud construit una vista en memoria de ses
  asignaciones de fragmentadores/carriles comprometidos. El administrador expone las instantáneas de solo lectura para la televisión
  (`GET /v2/sorafs/capacity/state`) y apliques de reservas por perfil o por carril delante de
  Los nuevos comandos no son aceptados.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- El punto final `/v2/sorafs/capacity/schedule` acepta las cargas útiles `ReplicationOrderV1` emitidas por el gobierno.
  Cuando el pedido se haga efectivo con el proveedor local, el administrador verificará la planificación en doble, validará la
  capacidad fragmentada/carril, reserve el tramo y envíe un `ReplicationPlan` decrivant la capacidad restante
  afin que les utilils d'orchestration puissent poursuivre l'ingestion. Las órdenes para otros proveedores
  Sont adquiridos con una respuesta `ignored` para facilitar los flujos de trabajo de múltiples operadores. 【crates/iroha_torii/src/routing.rs:4845】
- Des hooks de complétion (par ex. déclenchés après succès d'ingestion) apelante
  `POST /v2/sorafs/capacity/complete` para liberar las reservas a través de `CapacityManager::complete_order`.
  La respuesta incluye una instantánea `ReplicationRelease` (totaux restants, résiduels fragmenter/lane) después de
  les outils d'orchestration puissent queue la commande suivante sans polling. Un trabajo futuro relieracela au pipeline de chunk store lorsque la lógica de ingestión será prête.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- El `TelemetryAccumulator` embarqué peut être muté vía `NodeHandle::update_telemetry`, permiso auxiliar
  trabajadores de fondo de registro de échantillons PoR/uptime y de derivación de cargas útiles canónicas
  `CapacityTelemetryV1` sin contacto auxiliar interno del planificador.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integraciones y trabajos futuros

- **Gobernanza**: étendre `sorafs_pin_registry_tracker.md` con la télémétrie de stockage
  (taux de succès PoR, utilización disque). Las políticas de admisión pueden exigir una capacidad
  minimale ou un taux de succès PoR minimal avant d'accepter des adverts.
- **Clientes SDK**: expone la nueva configuración de almacenamiento (límites de disco, alias) para que
  les herramientas de gestión puissent bootstrapper les nœuds par program.
- **Télémétrie**: integrado con la pila de métricas existentes (Prometheus /
  OpenTelemetry) según las mediciones de almacenamiento que aparecen en los paneles de observación.
- **Seguridad**: ejecuta el módulo de almacenamiento en un grupo de tareas asíncrono descargado con contrapresión
  Y prevemos el sandboxing de las conferencias de trozos a través de io_uring o des pools tokio bornés para empêcher
  des client malveillants d'épuiser les resources.Este diseño mantiene el módulo de almacenamiento opcional y determina todo en donnant aux operadores les
Botones necesarios para participar en el sofá de disponibilidad de los donantes SoraFS. Implementación del hijo
implica cambios en `iroha_config`, `iroha_core`, `iroha_torii` y la puerta de enlace Norito, además
que les utiles d'advertir del proveedor.