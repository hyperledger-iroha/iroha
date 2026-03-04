---
lang: es
direction: ltr
source: docs/source/mochi/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44faf6c98d141959cf8cf40b1df7d3d82c3448e6f2b1bc4fa54cdeceb97994b0
source_last_modified: "2026-01-03T18:07:56.999063+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Inicio rápido de MOCHI

**MOCHI** es el supervisor de escritorio para redes locales Hyperledger Iroha. Esta guía recorre
instalar los requisitos previos, compilar la aplicación, iniciar egui shell y usar el
Herramientas de tiempo de ejecución (configuraciones, instantáneas, borrados) para el desarrollo diario.

## Requisitos previos

- Cadena de herramientas de Rust: `rustup default stable` (edición de objetivos del espacio de trabajo 2024 / Rust 1.82+).
- Cadena de herramientas de la plataforma:
  - macOS: Herramientas de línea de comandos de Xcode (`xcode-select --install`).
  - Linux: GCC, pkg-config, encabezados OpenSSL (`sudo apt install build-essential pkg-config libssl-dev`).
- Dependencias del espacio de trabajo Iroha:
  - `cargo xtask mochi-bundle` requiere `irohad`, `kagami` e `iroha_cli` integrados. Constrúyalos una vez a través de
    `cargo build -p irohad -p kagami -p iroha_cli`.
- Opcional: `direnv` o `cargo binstall` para gestionar binarios de carga locales.

MOCHI desembolsa los binarios de CLI. Asegúrese de que sean detectables a través de las variables de entorno.
a continuación o disponible en la RUTA:

| Binario | Anulación del entorno | Notas |
|----------|----------------------|-----------------------------------------|
| `irohad` | `MOCHI_IROHAD` | Supervisa a sus compañeros |
| `kagami` | `MOCHI_KAGAMI` | Genera manifiestos/instantáneas de génesis |
| `iroha_cli` | `MOCHI_IROHA_CLI` | Opcional para las próximas funciones auxiliares |

## Construyendo MOCHI

Desde la raíz del repositorio:

```bash
cargo build -p mochi-ui-egui
```

Este comando construye tanto `mochi-core` como la interfaz egui. Para producir un paquete distribuible, ejecute:

```bash
cargo xtask mochi-bundle
```

La tarea del paquete ensambla los archivos binarios, el manifiesto y la configuración en `target/mochi-bundle`.

## Lanzando el shell egui

Ejecute la interfaz de usuario directamente desde cargo:

```bash
cargo run -p mochi-ui-egui
```

De forma predeterminada, MOCHI crea un ajuste preestablecido de un solo par en un directorio de datos temporal:

- Raíz de datos: `$TMPDIR/mochi`.
- Puerto base Torii: `8080`.
- Puerto base P2P: `1337`.

Utilice indicadores CLI para anular los valores predeterminados al iniciar:

```bash
cargo run -p mochi-ui-egui -- \
  --data-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

Las variables de entorno reflejan las mismas anulaciones cuando se omiten los indicadores CLI: establezca `MOCHI_DATA_ROOT`,
`MOCHI_PROFILE`, `MOCHI_CHAIN_ID`, `MOCHI_TORII_START`, `MOCHI_P2P_START`, `MOCHI_RESTART_MODE`,
`MOCHI_RESTART_MAX` o `MOCHI_RESTART_BACKOFF_MS` para preconfigurar el constructor supervisor; caminos binarios
seguir respetando los puntos `MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI`, y `MOCHI_CONFIG` en un
explícito `config/local.toml`.

## Configuración y persistencia

Abra el cuadro de diálogo **Configuración** desde la barra de herramientas del panel para ajustar la configuración del supervisor:

- **Raíz de datos**: directorio base para configuraciones, almacenamiento, registros e instantáneas de pares.
- **Torii / Puertos base P2P**: puertos iniciales para asignación determinista.
- **Visibilidad del registro**: alterna los canales stdout/stderr/system en el visor de registros.

Perillas avanzadas como la política de reinicio del supervisor se encuentran en
`config/local.toml`. Establezca `[supervisor.restart] mode = "never"` para desactivar
reinicios automáticos durante la depuración de incidentes o ajuste
`max_restarts`/`backoff_ms` (a través del archivo de configuración o de los indicadores CLI
`--restart-mode`, `--restart-max`, `--restart-backoff-ms`) para controlar el reintento
comportamiento.La aplicación de cambios reconstruye el supervisor, reinicia los pares en ejecución y escribe las anulaciones en
`config/local.toml`. La combinación de configuración conserva claves no relacionadas para que los usuarios avanzados puedan mantener
ajustes manuales junto con valores administrados por MOCHI.

## Instantáneas y borrado/regénesis

El cuadro de diálogo **Mantenimiento** expone dos operaciones de seguridad:

- **Exportar instantánea**: copia el almacenamiento/config/logs del mismo nivel y el manifiesto de génesis actual en
  `snapshots/<label>` bajo la raíz de datos activa. Las etiquetas se desinfectan automáticamente.
- **Restaurar instantánea**: rehidrata el almacenamiento de pares, las raíces de instantáneas, las configuraciones, los registros y la génesis.
  manifiesto de un paquete existente. `Supervisor::restore_snapshot` acepta una ruta absoluta o
  el nombre de la carpeta desinfectada `snapshots/<label>`; la interfaz de usuario refleja este flujo, por lo que Mantenimiento → Restaurar
  Puede reproducir paquetes de pruebas sin tocar los archivos manualmente.
- **Wipe & re-genesis**: detiene la ejecución de pares, elimina directorios de almacenamiento, regenera genesis mediante
  Kagami y reinicia los pares cuando se completa el borrado.

Ambos flujos están cubiertos por pruebas de regresión (`export_snapshot_captures_storage_and_metadata`,
`wipe_and_regenerate_resets_storage_and_genesis`) para garantizar salidas deterministas.

## Registros y transmisiones

El panel expone datos/métricas de un vistazo:

- **Registros**: sigue los mensajes `irohad` stdout/stderr/ciclo de vida del sistema. Alternar canales en Configuración.
- **Bloques/Eventos**: las transmisiones administradas se reconectan automáticamente con retroceso exponencial y anotan fotogramas
  con resúmenes decodificados Norito.
- **Estado**: sondea `/status` y genera minigráficos para determinar la profundidad, el rendimiento y la latencia de la cola.
- **Preparación de inicio**: después de presionar **Iniciar** (un solo par o todos los pares), MOCHI sondea
  `/status` con retroceso limitado; el banner informa cuando cada par está listo (con el observado
  profundidad de la cola) o muestra el error Torii si se agota el tiempo de preparación.

Las pestañas para el explorador y compositor de estados brindan acceso rápido a cuentas, activos, pares y elementos comunes.
instrucciones sin salir de la interfaz de usuario. La vista Peers refleja la consulta `FindPeers` para que pueda confirmar
qué claves públicas están actualmente registradas en el conjunto de validadores antes de ejecutar las pruebas de integración.

Utilice el botón **Administrar bóveda de firmas** de la barra de herramientas del compositor para importar o editar autoridades de firma. el
El cuadro de diálogo escribe entradas en la raíz de la red activa (`<data_root>/<profile>/signers.json`) y las guarda.
Las claves de la bóveda están disponibles de inmediato para vistas previas y envíos de transacciones. Cuando la bóveda está
vacío, el compositor recurre a las claves de desarrollo incluidas para que los flujos de trabajo locales sigan funcionando.
Los formularios ahora cubren mint/burn/transfer (incluida la recepción implícita), dominio/cuenta/definición de activos
registro, políticas de admisión de cuentas, propuestas multifirma, manifiestos de directorio espacial (AXT/AMX),
Manifiestos de pin SoraFS y acciones de gobernanza como otorgar o revocar roles tan comunes
Las tareas de creación de hojas de ruta se pueden ensayar sin escribir a mano las cargas útiles Norito.

## Limpieza y solución de problemas- Detener la aplicación para despedir a los pares supervisados.
- Elimine la raíz de datos (`rm -rf <data_root>`) para restablecer todos los estados.
- Si cambian las ubicaciones de Kagami o irohad, actualice las variables de entorno o vuelva a ejecutar MOCHI con el
  banderas CLI apropiadas; el cuadro de diálogo Configuración mantendrá las nuevas rutas en la próxima aplicación.

Para automatización adicional, consulte `mochi/mochi-core/tests` (pruebas de ciclo de vida del supervisor) y
`mochi/mochi-integration` para escenarios simulados de Torii. Para enviar paquetes o cablear el
escritorio en canalizaciones de CI, consulte la guía {doc}`mochi/packaging`.

## Puerta de prueba local

Ejecute `ci/check_mochi.sh` antes de enviar parches para que la puerta CI compartida ejercite los tres MOCHI
cajas:

```bash
./ci/check_mochi.sh
```

El asistente ejecuta `cargo check`/`cargo test` para `mochi-core`, `mochi-ui-egui` y
`mochi-integration`, que detecta la deriva del dispositivo (capturas de eventos/bloques canónicos) y el arnés egui
regresiones de una sola vez. Si el script informa accesorios obsoletos, vuelva a ejecutar las pruebas de regeneración ignoradas,
por ejemplo:

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

Volver a ejecutar la puerta después de la regeneración garantiza que los bytes actualizados permanezcan consistentes antes de presionar.