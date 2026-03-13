---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-storage.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: almacenamiento de nodo
título: Diseño de almacenamiento del nodo SoraFS
sidebar_label: Diseño de almacenamiento del nodo
descripción: Arquitetura de almacenamiento, cuotas y ganchos de ciclo de vida para nodos Torii que hospedam dados SoraFS.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/sorafs_node_storage.md`. Mantenha ambas como copias sincronizadas.
:::

## Diseño de almacenamiento del nodo SoraFS (Borrador)

Esta nota refina como un nodo Iroha (Torii) pode optar pela camada de
disponibilidad de datos do SoraFS e dedicar um pedaco do disco local para
armazenar y servir trozos. Ela complementa una especificación de descubrimiento.
`sorafs_node_client_protocol.md` y el trabajo de accesorios SF-1b ao detalhar a
arquitectura del lado del almacenamiento, controles de recursos y plomería de
Configuracao que devem aterrissar no nodo e nos caminhos do gateway.
Os ejercicios operacionais ficam no
[Runbook de operaciones del nodo](./node-operations).

### Objetivos- Permitir que cualquier validador o proceso auxiliar del disco exponente Iroha
  ocioso como proveedor SoraFS sem afetar as responsabilidades do ledger.
- Manter o módulo de almacenamiento determinístico y guiado por Norito: manifiestos,
  planos de trozos, plantea prueba de recuperabilidad (PoR) y anuncios de proveedor sao
  una fuente de verdad.
- Impor cuotas definidas por el operador para que un nodo nao esgote seus
  recursos para aceitar demasiados requisitos de pin ou fetch.
- Export saude/telemetria (amostragem PoR, latencia de fetch de chunk, pressao de
  disco) de vuelta para gobierno y clientes.

### Arquitectura de alto nivel

```
+--------------------------------------------------------------------+
|                         Iroha/Torii Node                           |
|                                                                    |
|  +----------+      +----------------------+                        |
|  | Torii APIs|<---->|    SoraFS Gateway   |<---------------+       |
|  +----------+      |  (Norito endpoints)  |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |     Pin Registry     |<---- manifests |       |
|                    |     (State / DB)     |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |     Chunk Storage    |<---- chunk plans|       |
|                    |      (ChunkStore)    |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |    Disk Quota/IO     |--pin/serve----->| Fetch |
|                    |      Scheduler       |                | Clients|
|                    +----------------------+                |       |
|                                                                    |
+--------------------------------------------------------------------+
```

Módulos chave:- **Gateway**: expoe endpoints HTTP Norito para propuestas de pin, requisitos de
  buscar fragmentos, amostragem PoR y telemetria. Cargas útiles de validación Norito e
  encaminha requisicoes para o chunk store. Reutilizar la pila HTTP de Torii para
  Evitar un nuevo demonio.
- **Registro de PIN**: estado de pin de manifiesto rastreado en `iroha_data_model::sorafs`
  y `iroha_core`. Quando um manifest e aceito o registro registro o digest do
  manifest, digest do plano de chunk, raiz PoR e flags de capacidade do provedor.
- **Almacenamiento de fragmentos**: implementamos `ChunkStore` en disco que contiene manifiestos
  assinados, materializa planos de chunk usando `ChunkProfile::DEFAULT`, e
  persistir fragmentos en diseño determinístico. Cada trozo y asociado a um
  huellas dactilares de conteudo e metadados PoR para que a amostragem possa revalidar
  sem reler o arquivo inteiro.
- **Cuota/Programador**: impone límites configurados por el operador (bytes max de
  disco, pines colgantes max, búsquedas paralelas max, TTL de chunk) y coordenadas IO
  para que as tarefas do ledger nao fiquem sem recursos. O planificador tambem e
  Responsavel por servir provas PoR e requisicoes de amostragem com CPU limitada.

### Configuración

Agregue una nueva seca en `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # tag opcional legivel
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```- `enabled`: alternar participación. Quando false o gateway retorna 503 para
  endpoints de almacenamiento y el nodo nao anuncia em descubrimiento.
- `data_dir`: directorio raiz para dados de chunk, arvores PoR e telemetria de
  buscar. Predeterminado `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: límite rígido para dados de trozos pinados. Uma tarefa de
  fundo rejeita novos pins quando o limite e atingido.
- `max_parallel_fetches`: límite de concordancia impuesto por el planificador para
  balancear banda/IO de disco contra a carga do validador.
- `max_pins`: máximo de pines de manifiesto que o nodo aceita antes de aplicar
  desalojo/contrapresión.
- `por_sample_interval_secs`: cadencia para trabajos automáticos de amostragem PoR.
  Cada trabajo mostramos `N` folhas (configuravel por manifest) y emite eventos de
  telemetría. Agobernanza pode escalar `N` de forma determinística definindo a
  chave de metadata `profile.sample_multiplier` (inteiro `1-4`). Oh valor pode
  ser un número/cadena único o un objeto con anulaciones por perfil, por ejemplo
  `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estructura usada pelo gerador de advert para preencher campos
  `ProviderAdvertV1` (puntero de participación, sugerencias de QoS, temas). Se omitió o nodo
  Estados Unidos por defecto hace el registro de gobierno.

Configuración de plomería:- `[sorafs.storage]` y definido en `iroha_config` como `SorafsStorage` e e
  cargado del archivo de configuración del nodo.
- `iroha_core` e `iroha_torii` pasan a la configuración de almacenamiento para el constructor
  hacer puerta de enlace y almacén de fragmentos sin inicio.
- Anula la existencia de desarrollo/prueba (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), mas
  Los despliegues de producción deben confiar en el archivo de configuración.

### Utilitarios de CLI

Enquanto a superficie HTTP do Torii ainda esta sendo ligada, o crate
`sorafs_node` envia una CLI leve para que operadores possam automatizar taladros de
ingesta/exportación contra el backend persistente. [cajas/sorafs_node/src/bin/sorafs-node.rs:1]

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` espera un manifiesto `.to` codificado en Norito más bytes de carga útil
  corresponsales. Ele reconstroi o plano de trozo a partir del perfil de
  fragmentación se manifiesta, impone paridade de digest, persiste archivos de fragment e
  Opcionalmente emite un blob JSON `chunk_fetch_specs` para herramientas posteriores.
  validar el diseño.
- `export` aceita un ID de manifiesto y graba el manifiesto/carga útil armado en
  disco (con plan JSON opcional) para que los aparatos continúen reproduciéndose.Ambos comandos imprimen un resumen Norito JSON en stdout, facilitando su uso
guiones. A CLI e coberta por um teste de integracao para garantizar que manifiesta e
Las cargas útiles hacen un viaje de ida y vuelta correctamente antes de que las API Torii entren. [crates/sorafs_node/tests/cli.rs:1]

> Paridad HTTP
>
> O gateway Torii ahora expoe helpers de solo lectura apoiados pelo mesmo
> `NodeHandle`:
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` - retorna o manifiesto
> Norito armado (base64) junto con digest/metadata. [crates/iroha_torii/src/sorafs/api.rs:1207]
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` - retorna o plano de trozo
> JSON determinístico (`chunk_fetch_specs`) para herramientas posteriores. [crates/iroha_torii/src/sorafs/api.rs:1259]
>
> Estos puntos finales se envían a la CLI para que las tuberías tengan un trocar
> scripts ubicados en sondas HTTP sin cambiar analizadores. [crates/iroha_torii/src/sorafs/api.rs:1207] [crates/iroha_torii/src/sorafs/api.rs:1259]

### Ciclo de vida del nodo1. **Inicio**:
   - Si el almacenamiento estiver habilitado o nodo inicializa o chunk store com o
     directorio y capacidade configurados. Esto incluye verificar o criar una base.
     de dados de manifest PoR e fazer replay de manifests pinados para aquecer
     cachés.
   - Registrador como rotas de la puerta de enlace SoraFS (puntos finales Norito JSON POST/GET para pin,
     buscar, amostragem PoR, telemetría).
   - Iniciar o trabajador de amostragem PoR e o monitor de cuotas.
2. **Descubrimiento/Anuncios**:
   - Gerar documentos `ProviderAdvertV1` usando capacidade/saude atual, assinar
     com a chave aprobado pelo conselho e publicar vía descubrimiento. Usa una lista
3. **Flujo de pin**:
   - O gateway recibe un manifiesto assinado (incluindo plano de chunk, raiz PoR,
     assinaturas do conselho). Valida una lista de alias (`sorafs.sf1@1.0.0`
     requerido) y garantiza que el plano de fragmento corresponde a los metadatos manifiestos.
   - Verifica cuotas. Se capacidade/limites de pin excederem, responde com error
     de politica (Norito estructurado).
   - Stream de dados de chunk para o `ChunkStore`, verificando resúmenes durante
     una ingesta. Los metadatos de Atualiza arvores PoR y armazena no manifiestan ningún registro.
4. **Flujo de búsqueda**:
   - Sirva los requisitos de variedad de trozos desde la discoteca. Oh planificador impone
     `max_parallel_fetches` y retorna `429` cuando está saturado.- Emite telemetría estructurada (Norito JSON) con latencia, bytes servidos y
     contadores de error para monitoreo aguas abajo.
5. **Amostragem PoR**:
   - La selección de trabajadores se manifiesta proporcionalmente al peso (por ejemplo, bytes
     armazenados) e roda amostragem deterministica usando arvore PoR do chunk store.
   - Persiste resultados para auditorias de gobierno e incluye resúmenes en anuncios.
     de proveedor/endpoints de telemetria.
6. **Desalojo / cumplimiento de cuotas**:
   - Quando a capacidade e atingida o nodo rejeita novos pins por padrao. Opcionalmente,
     Los operadores pueden configurar políticas de desalojo (ej: TTL, LRU) cuando el modelo
     de gobernancia estiver definida; por ora o diseño asumir cuotas estritas e
     operaciones de desvinculación iniciadas por el operador.

### Declaración de capacidad e integración de programación- Torii ahora actualiza las actualizaciones de `CapacityDeclarationRecord` de `/v2/sorafs/capacity/declare`
  para o `CapacityManager` embutido, de modo que cada nodo constroi uma visao em memoria
  de sus alojamientos comprometidos de chunker e lane. O manager expone instantáneas de solo lectura
  para telemetría (`GET /v2/sorafs/capacity/state`) e imponer reservas por perfil o carril
  antes de novas ordens serem aceitas. [crates/sorafs_node/src/capacity.rs:1] [crates/sorafs_node/src/lib.rs:60]
- O endpoint `/v2/sorafs/capacity/schedule` cargas útiles aceita `ReplicationOrderV1`
  emitidos pelagobernanza. Quando a ordem mira o provedor local o manager verifica
  programación duplicada, valida capacidade de chunker/lane, reserva o slice e retorna
  um `ReplicationPlan` descrevendo la capacidad restante para que ferramentas de
  orquestracao possam seguir com a ingestao. Ordenes para otros proveedores sao
  reconhecidas com resposta `ignored` para facilitar flujos de trabajo multioperador. [crates/iroha_torii/src/routing.rs:4845]
- Hooks de conclusao (por ejemplo, disparados apos ingestao bem sucedida) chamam
  `POST /v2/sorafs/capacity/complete` para liberar reservas vía
  `CapacityManager::complete_order`. Una respuesta que incluye una instantánea.
  `ReplicationRelease` (totalmente restantes, residuos de fragmentador/carril) para herramientas
  de orquestracao possa enfileirar a proxima ordem sem polling. Trabajo futuro
  Vai conectar esto a la tubería del almacén de trozos así como una lógica de ingestao chegar.[crates/iroha_torii/src/routing.rs:4885] [crates/sorafs_node/src/capacity.rs:90]
- O `TelemetryAccumulator` embutido pode ser mutado vía
  `NodeHandle::update_telemetry`, permitiendo que los trabajadores se registren en segundo plano
  demostraciones de PoR/uptime y eventualmente derivar cargas útiles canónicas
  `CapacityTelemetryV1` Sem toca los internos del planificador. [crates/sorafs_node/src/lib.rs:142] [crates/sorafs_node/src/telemetry.rs:1]

### Integracoes e trabalho futuro

- **Gobernanza**: extender `sorafs_pin_registry_tracker.md` con telemetria de
  almacenamiento (taxa de sucesso PoR, utilizacao de disco). Políticas de admisión podem
  Requerir capacidad mínima o taxa mínima de éxito PoR antes de aceptar anuncios.
- **SDK de cliente**: exporta una nueva configuración de almacenamiento (limites de disco, alias)
  Para que las herramientas de gestao puedan arrancar nodos programáticamente.
- **Telemetría**: integrar con la pila de métricas existentes (Prometheus /
  OpenTelemetry) para que las métricas de almacenamiento aparezcan en paneles de observabilidad.
- **Seguranca**: rodar el módulo de almacenamiento en un pool dedicado de tarefas async com
  contrapresión considerar y sandboxing de leituras de chunk a través de io_uring o pools
  tokio limitados para evitar que clientes maliciosos esgotem recursos.Este diseño mantem o módulo de almacenamiento opcional y determinístico en cuanto a necesidad.
os perillas necesarias para operadores participarem da camada de disponibilidad de dados
SoraFS. Implemente lo requerido para mudarse en `iroha_config`, `iroha_core`, `iroha_torii`
Si no hay puerta de enlace Norito, también puede utilizar el anuncio del proveedor.