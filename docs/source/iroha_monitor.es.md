---
lang: es
direction: ltr
source: docs/source/iroha_monitor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05149d624d680d04433be41a4525538c97bd103ae7f80dda2613a6adb181a93d
source_last_modified: "2026-01-03T18:07:57.206662+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Monitor Iroha

El monitor Iroha refactorizado combina una interfaz de usuario de terminal liviana con animaciones
festival de arte ASCII y el tema tradicional Etenraku.  Se centra en dos
flujos de trabajo simples:

- **Modo Spawn-lite**: inicia resúmenes efímeros de estado/métricas que imitan a sus pares.
- **Modo de conexión**: apunte el monitor a los puntos finales HTTP Torii existentes.

La interfaz de usuario muestra tres regiones en cada actualización:

1. **Torii encabezado del horizonte**: puerta torii animada, monte Fuji, olas koi y estrella
   campo que se desplaza en sincronía con la cadencia de actualización.
2. **Tira de resumen**: bloques/transacciones/gas agregados más tiempo de actualización.
3. **Mesa de pares y susurros de festivales**: filas de pares a la izquierda, evento rotativo
   inicie sesión a la derecha que captura advertencias (tiempos de espera, cargas útiles de gran tamaño, etc.).
4. **Tendencia de gas opcional**: habilite `--show-gas-trend` para agregar un minigráfico
   resumiendo el uso total de gas en todos los pares.

Nuevo en esta refactorización:

- Escena animada ASCII de estilo japonés con koi, torii y linternas.
- Superficie de mando simplificada (`--spawn-lite`, `--attach`, `--interval`).
- Banner de introducción con reproducción de audio opcional del tema gagaku (MIDI externo
  reproductor o el sintetizador de software incorporado cuando la plataforma/pila de audio lo admite).
- Banderas `--no-theme` / `--no-audio` para CI o carreras de humo rápidas.
- Columna de "estado de ánimo" por igual que muestra la última advertencia, tiempo de confirmación o tiempo de actividad.

## Inicio rápido

Construya el monitor y ejecútelo contra los pares eliminados:

```bash
cargo run -p iroha_monitor -- --spawn-lite --peers 3
```

Adjunte a los puntos finales Torii existentes:

```bash
cargo run -p iroha_monitor -- \
  --attach http://127.0.0.1:8080 http://127.0.0.1:8081 \
  --interval 500
```

Invocación compatible con CI (omita la animación de introducción y el audio):

```bash
cargo run -p iroha_monitor -- --spawn-lite --no-theme --no-audio
```

### Banderas CLI

```
--spawn-lite         start local status/metrics stubs (default if no --attach)
--attach <URL...>    attach to existing Torii endpoints
--interval <ms>      refresh interval (default 800ms)
--peers <N>          stub count when spawn-lite is active (default 4)
--no-theme           skip the animated intro splash
--no-audio           mute theme playback (still prints the intro frames)
--midi-player <cmd>  external MIDI player for the built-in Etenraku .mid
--midi-file <path>   custom MIDI file for --midi-player
--show-gas-trend     render the aggregate gas sparkline panel
--art-speed <1-8>    multiply the animation step rate (1 = default)
--art-theme <name>   choose between night, dawn, or sakura palettes
--headless-max-frames <N>
                     cap headless fallback to N frames (0 = unlimited)
```

## Introducción al tema

De forma predeterminada, el inicio reproduce una breve animación ASCII mientras la puntuación Etenraku
comienza.  Orden de selección de audio:

1. Si se proporciona `--midi-player`, genere el MIDI de demostración (o use `--midi-file`)
   y generar el comando.
2. De lo contrario, en macOS/Windows (o Linux con `--features iroha_monitor/linux-builtin-synth`)
   renderice la partitura con el sintetizador suave gagaku incorporado (sin audio externo)
   bienes necesarios).
3. Si el audio está desactivado o la inicialización falla, la introducción aún imprime el
   animación e inmediatamente ingresa a la TUI.

El sintetizador con tecnología CPAL se habilita automáticamente en macOS y Windows. En Linux es
opte por evitar que se pierdan encabezados ALSA/Pulse durante la creación del espacio de trabajo; habilitarlo
con `--features iroha_monitor/linux-builtin-synth` si su sistema proporciona un
pila de audio en funcionamiento.

Utilice `--no-theme` o `--no-audio` cuando ejecute CI o shells sin cabeza.

El sintetizador suave ahora sigue el arreglo capturado en el diseño del sintetizador *MIDI en
Rust.pdf*: hichiriki y ryūteki comparten una melodía heterofónica mientras el shō
proporciona las almohadillas de aitake descritas en el documento.  Los datos de la nota cronometrada viven
en `etenraku.rs`; alimenta tanto la devolución de llamada CPAL como la demostración MIDI generada.
Cuando la salida de audio no está disponible, el monitor omite la reproducción pero aún muestra
la animación ASCII.

## descripción general de la interfaz de usuario- **Arte del encabezado**: generó cada cuadro mediante `AsciiAnimator`; koi, linternas torii,
  y las olas se desplazan para dar un movimiento continuo.
- **Banda de resumen**: muestra los pares en línea, el recuento de pares informado, los totales de bloques,
  totales de bloques no vacíos, aprobaciones/rechazos de tx, uso de gas y frecuencia de actualización.
- **Tabla de pares**: columnas para alias/punto final, bloques, transacciones, tamaño de cola,
  uso de gas, latencia y una pista de "estado de ánimo" (advertencias, tiempo de confirmación, tiempo de actividad).
- **Susurros de festivales**: registro continuo de advertencias (errores de conexión, carga útil
  infracciones de límites, puntos finales lentos).  Los mensajes están invertidos (el último está arriba).

Atajos de teclado:

- `n` / Derecha / Abajo: mueve el foco al siguiente par.
- `p` / Izquierda / Arriba: mueve el foco al par anterior.
- `q` / Esc / Ctrl-C – salir y restaurar la terminal.

El monitor utiliza crossterm + rattatui con un búfer de pantalla alternativa; al salir
restaura el cursor y borra la pantalla.

## Pruebas de humo

La caja incluye pruebas de integración que ejercitan ambos modos y los límites de HTTP:

- `spawn_lite_smoke_renders_frames`
- `attach_mode_with_stubs_runs_cleanly`
- `invalid_endpoint_surfaces_warning`
- `status_limit_warning_is_rendered`
- `attach_mode_with_slow_peer_renders_multiple_frames`

Ejecute solo las pruebas del monitor:

```bash
cargo test -p iroha_monitor -- --nocapture
```

El espacio de trabajo tiene pruebas de integración más exigentes (`cargo test --workspace`). corriendo
Las pruebas del monitor por separado siguen siendo útiles para una validación rápida cuando lo haces.
No necesito la suite completa.

## Actualizando capturas de pantalla

La demostración de documentos ahora se centra en el horizonte torii y la mesa de pares.  Para refrescar el
activos, ejecute:

```bash
make monitor-screenshots
```

Esto incluye `scripts/iroha_monitor_demo.sh` (modo spawn-lite, semilla/ventana gráfica fija,
sin introducción/audio, paleta del amanecer, velocidad artística 1, tapa sin cabeza 24) y escribe el
Marcos SVG/ANSI más `manifest.json` e `checksums.json` en
`docs/source/images/iroha_monitor_demo/`. `make check-iroha-monitor-docs`
envuelve ambos protectores CI (`ci/check_iroha_monitor_assets.sh` y
`ci/check_iroha_monitor_screenshots.sh`) por lo que los hashes del generador, los campos de manifiesto,
y las sumas de verificación permanecen sincronizadas; la verificación de captura de pantalla también se envía como
`python3 scripts/check_iroha_monitor_screenshots.py`. Pase `--no-fallback` a
el script de demostración si desea que la captura falle en lugar de recurrir al
fotogramas horneados cuando la salida del monitor está vacía; cuando se utiliza el respaldo en bruto
Los archivos `.ans` se reescriben con los marcos horneados para que el manifiesto/las sumas de verificación permanezcan
determinista.

## Capturas de pantalla deterministas

Las instantáneas enviadas se encuentran en `docs/source/images/iroha_monitor_demo/`:

![descripción general del monitor](images/iroha_monitor_demo/iroha_monitor_demo_overview.svg)
![monitorear canalización](images/iroha_monitor_demo/iroha_monitor_demo_pipeline.svg)

Reproducirlos con una ventana gráfica/semilla fija:

```bash
scripts/iroha_monitor_demo.sh \
  --cols 120 --rows 48 \
  --interval 500 \
  --seed iroha-monitor-demo
```

El asistente de captura corrige `LANG`/`LC_ALL`/`TERM`, reenvía
`IROHA_MONITOR_DEMO_SEED`, silencia el audio y fija el tema/velocidad artística para que
Los marcos se representan de manera idéntica en todas las plataformas. Escribe `manifest.json` (generador
hashes + tamaños) y `checksums.json` (resúmenes SHA-256) en
`docs/source/images/iroha_monitor_demo/`; ejecuciones de CI
`ci/check_iroha_monitor_assets.sh` y `ci/check_iroha_monitor_screenshots.sh`
fracasar cuando los activos se desvían de los manifiestos registrados.

## Solución de problemas- **Sin salida de audio**: el monitor vuelve a la reproducción silenciada y continúa.
- **El respaldo sin cabeza sale temprano**: el monitor limita las ejecuciones sin cabeza a un par
  docena de cuadros (aproximadamente 12 segundos en el intervalo predeterminado) cuando no puede cambiar
  el terminal en modo crudo; pase `--headless-max-frames 0` para mantenerlo funcionando
  indefinidamente.
- **Cargas útiles de estado de gran tamaño**: la columna de estado de ánimo del compañero y el registro del festival
  muestra `body exceeds …` con el límite configurado (`128 KiB`).
- **Pares lentos**: el registro de eventos registra advertencias de tiempo de espera; enfocar a ese compañero a
  resaltar la fila.

¡Disfruta del horizonte del festival!  Contribuciones para motivos ASCII adicionales o
Los paneles de métricas son bienvenidos: manténgalos deterministas para que los grupos representen lo mismo.
cuadro por cuadro independientemente del terminal.