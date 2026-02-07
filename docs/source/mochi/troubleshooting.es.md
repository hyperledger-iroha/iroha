---
lang: es
direction: ltr
source: docs/source/mochi/troubleshooting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb67b304bae01fa4a50d25dc9f086811dabfbcb24239b3ec9679338248e18be6
source_last_modified: "2026-01-03T18:07:57.000591+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Guía de solución de problemas de MOCHI

Utilice este runbook cuando los clústeres MOCHI locales se nieguen a iniciarse, queden atrapados en un
reiniciar el ciclo o detener la transmisión de actualizaciones de bloque/evento/estado. Se extiende el
elemento de la hoja de ruta "Documentación e implementación" al convertir los comportamientos del supervisor en
`mochi-core` en pasos concretos de recuperación.

## 1. Lista de verificación de primeros auxilios

1. Capture la raíz de datos que está utilizando MOCHI. El valor predeterminado sigue
   `$TMPDIR/mochi/<profile-slug>`; Las rutas personalizadas aparecen en la barra de título de la interfaz de usuario y
   vía `cargo run -p mochi-ui-egui -- --data-root ...`.
2. Ejecute `./ci/check_mochi.sh` desde la raíz del espacio de trabajo. Esto valida el núcleo,
   UI y cajas de integración antes de comenzar a modificar las configuraciones.
3. Tenga en cuenta el valor predeterminado (`single-peer` o `four-peer-bft`). La topología generada
   determina cuántas carpetas/registros de pares debe esperar en la raíz de datos.

## 2. Recopilar registros y evidencia de telemetría

`NetworkPaths::ensure` (ver `mochi/mochi-core/src/config.rs`) crea un estable
diseño:

```
<data_root>/<profile>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
```

Siga estos pasos antes de realizar cambios:

- Utilice la pestaña **Registros** o abra `logs/<alias>.log` directamente para capturar el último
  200 líneas para cada par. El supervisor sigue los canales stdout/stderr/system
  a través de `PeerLogStream`, para que estos archivos coincidan con la salida de la interfaz de usuario.
- Exportar una instantánea a través de **Mantenimiento → Exportar instantánea** (o llamar
  `Supervisor::export_snapshot`). La instantánea agrupa almacenamiento, configuraciones y
  inicia sesión en `snapshots/<timestamp>-<label>/`.
- Si el problema involucra widgets de transmisión, copie el archivo `ManagedBlockStream`,
  Indicadores de estado `ManagedEventStream` e `ManagedStatusStream` del
  Panel de control. La interfaz de usuario muestra el último intento de reconexión y el motivo del error; agarrar
  una captura de pantalla para el registro del incidente.

## 3. Resolver problemas de inicio de pares

La mayoría de los errores de lanzamiento entre pares se dividen en tres grupos:

### Faltan archivos binarios o anulaciones incorrectas

`SupervisorBuilder` se transfiere a `irohad`, `kagami` y (futuro) `iroha_cli`.
Si la interfaz de usuario informa "no se pudo generar el proceso" o "permiso denegado", señale MOCHI
en binarios en buen estado:

```bash
cargo run -p mochi-ui-egui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

Puede configurar `MOCHI_IROHAD`, `MOCHI_KAGAMI` e `MOCHI_IROHA_CLI` para evitar
escribiendo las banderas repetidamente. Al depurar compilaciones de paquetes, compare el
`BundleConfig` en `mochi/mochi-ui-egui/src/config/` contra las rutas en
`target/mochi-bundle`.

### Colisiones portuarias

`PortAllocator` prueba la interfaz de bucle invertido antes de escribir las configuraciones. si ves
`failed to allocate Torii port` o `failed to allocate P2P port`, otro
El proceso ya está escuchando en el rango predeterminado (8080/1337). Relanzar MOCHI
con bases explícitas:

```bash
cargo run -p mochi-ui-egui -- --torii-start 12000 --p2p-start 19000
```

El constructor desplegará puertos secuenciales desde esas bases, así que reserve un rango
dimensionado para su valor preestablecido (iguales `peer_count` → puertos `peer_count` por transporte).

### Génesis y corrupción del almacenamientoSi Kagami sale antes de emitir un manifiesto, los pares fallarán inmediatamente. comprobar
`genesis/*.json`/`.toml` dentro de la raíz de datos. Vuelva a ejecutar con
`--kagami /path/to/kagami` o apunte el cuadro de diálogo **Configuración** al binario derecho.
Para daños en el almacenamiento, utilice **Limpiar y volver a generar** de la sección Mantenimiento.
botón (que se explica a continuación) en lugar de eliminar carpetas a mano; recrea el
directorios de pares y raíces de instantáneas antes de reiniciar los procesos.

### Ajuste de reinicios automáticos

`[supervisor.restart]` en `config/local.toml` (o los indicadores CLI
`--restart-mode`, `--restart-max`, `--restart-backoff-ms`) controlan la frecuencia con la que
El supervisor reintenta a los pares fallidos. Configure `mode = "never"` cuando necesite que la interfaz de usuario
sacar a la luz la primera falla inmediatamente o acortar `max_restarts`/`backoff_ms`
para ajustar la ventana de reintento para trabajos de CI que deben fallar rápidamente.

## 4. Restablecer pares de forma segura

1. Detenga a los pares afectados desde el Panel o salga de la interfaz de usuario. el supervisor
   se niega a borrar el almacenamiento mientras se ejecuta un par (`PeerHandle::wipe_storage`
   devuelve `PeerStillRunning`).
2. Navegue hasta **Mantenimiento → Limpiar y volver a generar**. MOCHI:
   - eliminar `peers/<alias>/storage`;
   - vuelva a ejecutar Kagami para reconstruir las configuraciones/génesis en `genesis/`; y
   - reinicie los pares con las anulaciones de entorno/CLI conservadas.
3. Si debes hacer esto manualmente:
   ```bash
   cargo run -p mochi-ui-egui -- --data-root /tmp/mochi --profile four-peer-bft --help
   # Note the actual root printed above, then:
   rm -rf /tmp/mochi/four-peer-bft
   ```
   Luego, reinicie MOCHI para que `NetworkPaths::ensure` vuelva a crear el árbol.

Archive siempre la carpeta `snapshots/<timestamp>` antes de borrarla, incluso en local
desarrollo: esos paquetes capturan los registros y configuraciones `irohad` precisos necesarios
para reproducir errores.

### 4.1 Restaurar a partir de instantáneas

Cuando un experimento daña el almacenamiento o necesita reproducir un estado en buen estado, utilice el Mantenimiento
botón **Restaurar instantánea** del cuadro de diálogo (o llame a `Supervisor::restore_snapshot`) en lugar de copiar
directorios manualmente. Proporcione una ruta absoluta al paquete o el nombre de la carpeta desinfectada
bajo `snapshots/`. El supervisor:

1. detener a los compañeros en ejecución;
2. verificar que el `metadata.json` de la instantánea coincida con el `chain_id` actual y el recuento de pares;
3. copie `peers/<alias>/{storage,snapshot,config.toml,latest.log}` nuevamente en el perfil activo; y
4. restaure `genesis/genesis.json` antes de reiniciar los pares si se estaban ejecutando de antemano.

Si la instantánea se creó para un identificador de cadena o preestablecido diferente, la llamada de restauración devuelve un
`SupervisorError::Config` para que puedas tomar un paquete que combine en lugar de mezclar artefactos en silencio.
Mantenga al menos una instantánea nueva por ajuste preestablecido para acelerar los ejercicios de recuperación.

## 5. Reparación de secuencias de bloques/evento/estado- **Transmisión detenida pero pares en buen estado.** Consulte los paneles **Eventos**/**Bloques**
  para barras de estado rojas. Haga clic en "Detener" y luego en "Iniciar" para forzar que la transmisión administrada
  volver a suscribirse; el supervisor registra cada intento de reconexión (con el alias del par y
  error) para que pueda confirmar las etapas de retroceso.
- **Superposición de estado desactualizada.** `ManagedStatusStream` sondea `/status` cada
  dos segundos y marca los datos obsoletos después de `STATUS_POLL_INTERVAL *
  STATUS_STALE_MULTIPLIER` (seis segundos predeterminados). Si la insignia permanece roja, verifique
  `torii_status_url` en la configuración de pares y asegúrese de que la puerta de enlace o VPN no esté
  bloqueando las conexiones loopback.
- **Errores de decodificación de eventos.** La interfaz de usuario imprime la etapa de decodificación (bytes sin formato,
  `BlockSummary` o Norito decodificación) y el hash de transacción infractor. Exportar
  el evento a través del botón del portapapeles para que pueda reproducir la decodificación en las pruebas
  (`mochi-core` expone constructores auxiliares en
  `mochi/mochi-core/src/torii.rs`).

Cuando las transmisiones fallan repetidamente, actualice el problema con el alias exacto del par y
cadena de error (`ToriiErrorKind`) para que los hitos de telemetría de la hoja de ruta permanezcan vinculados
a pruebas concretas.