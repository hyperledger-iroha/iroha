---
lang: es
direction: ltr
source: docs/source/mochi/packaging.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ab0877a6f43402d6ec13a44c4a7c2b68e4a49e6103bb50d7469d9e71aaa953
source_last_modified: "2026-01-03T18:07:57.001158+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Guía de embalaje de MOCHI

Esta guía explica cómo crear el paquete de supervisor de escritorio MOCHI, inspeccionar
los artefactos generados y ajustar las anulaciones de tiempo de ejecución que se incluyen con el
paquete. Complementa el inicio rápido centrándose en envases reproducibles.
y uso de CI.

## Requisitos previos

- Cadena de herramientas de Rust (edición 2024 / Rust 1.82+) con dependencias del espacio de trabajo
  ya construido.
- `irohad`, `iroha_cli` e `kagami` compilados para el objetivo deseado. el
  El paquete reutiliza archivos binarios de `target/<profile>/`.
- Espacio en disco suficiente para la salida del paquete en `target/` o un personalizado
  destino.

Cree las dependencias una vez antes de ejecutar el paquete:

```bash
cargo build -p irohad -p iroha_cli -p iroha_kagami
```

## Construyendo el paquete

Invoque el comando dedicado `xtask` desde la raíz del repositorio:

```bash
cargo xtask mochi-bundle
```

De forma predeterminada, esto produce un paquete de versión bajo `target/mochi-bundle/` con un
nombre de archivo derivado del sistema operativo y la arquitectura del host (por ejemplo,
`mochi-macos-aarch64-release.tar.gz`). Utilice las siguientes banderas para personalizar
la construcción:

- `--profile <name>`: elija un perfil de carga (`release`, `debug` o un
  perfil personalizado).
- `--no-archive` – mantiene el directorio expandido sin crear un `.tar.gz`
  archivo (útil para pruebas locales).
- `--out <path>` – escribe paquetes en un directorio personalizado en lugar de
  `target/mochi-bundle/`.
- `--kagami <path>`: proporcione un ejecutable `kagami` prediseñado para incluirlo en el
  archivo. Cuando se omite, el paquete reutiliza (o construye) el binario a partir del
  perfil seleccionado.
- `--matrix <path>`: agrega metadatos del paquete a un archivo de matriz JSON (creado si
  falta) para que las canalizaciones de CI puedan registrar cada artefacto de host/perfil producido en un
  correr. Las entradas incluyen el directorio del paquete, la ruta del manifiesto y SHA-256, opcional
  ubicación del archivo y el último resultado de la prueba de humo.
- `--smoke`: ejecute el `mochi --help` empaquetado como una puerta de humo liviana
  después de agrupar; Las fallas muestran dependencias faltantes antes de publicar un
  artefacto.
- `--stage <path>`: copie el paquete terminado (y archívelo cuando se produzca) en
  un directorio provisional para que las compilaciones multiplataforma puedan depositar artefactos en uno
  ubicación sin secuencias de comandos adicionales.

El comando copia `mochi-ui-egui`, `kagami`, `LICENSE`, el ejemplo
configuración e `mochi/BUNDLE_README.md` en el paquete. Un determinista
`manifest.json` se genera junto con los binarios para que los trabajos de CI puedan rastrear el archivo
hashes y tamaños.

## Diseño y verificación del paquete

Un paquete ampliado sigue el diseño documentado en `BUNDLE_README.md`:

```
bin/mochi
bin/kagami
config/sample.toml
docs/README.md
manifest.json
LICENSE
```

El archivo `manifest.json` enumera cada artefacto con su hash SHA-256. verificar
el paquete después de copiarlo a otro sistema:

```bash
jq -r '.files[] | "\(.sha256)  \(.path)"' manifest.json | sha256sum --check
```

Las canalizaciones de CI pueden almacenar en caché el directorio expandido, firmar el archivo o publicar
el manifiesto junto con las notas de la versión. El manifiesto incluye el generador.
perfil, triple objetivo y marca de tiempo de creación para facilitar el seguimiento de procedencia.

## Anulaciones de tiempo de ejecución

MOCHI descubre binarios auxiliares y ubicaciones de tiempo de ejecución a través de indicadores CLI o
variables de entorno:- `--data-root` / `MOCHI_DATA_ROOT`: anula el espacio de trabajo utilizado para el par
  configuraciones, almacenamiento y registros.
- `--profile` – cambiar entre ajustes preestablecidos de topología (`single-peer`,
  `four-peer-bft`).
- `--torii-start`, `--p2p-start`: cambie los puertos base utilizados al asignar
  servicios.
- `--irohad` / `MOCHI_IROHAD`: apunta a un binario `irohad` específico.
- `--kagami` / `MOCHI_KAGAMI`: anula el `kagami` incluido.
- `--iroha-cli` / `MOCHI_IROHA_CLI`: anula el asistente CLI opcional.
- `--restart-mode <never|on-failure>` – deshabilita los reinicios automáticos o fuerza el
  política de retroceso exponencial.
- `--restart-max <attempts>`: anula el número de intentos de reinicio cuando
  ejecutándose en modo `on-failure`.
- `--restart-backoff-ms <millis>`: establece el retroceso de la base para reinicios automáticos.
- `MOCHI_CONFIG`: proporciona una ruta `config/local.toml` personalizada.

La ayuda de CLI (`mochi --help`) imprime la lista de indicadores completa. Anulaciones de entorno
surtirá efecto al iniciar y se puede combinar con el cuadro de diálogo Configuración dentro del
Interfaz de usuario.

## Consejos de uso de CI

- Ejecute `cargo xtask mochi-bundle --no-archive` para generar un directorio que pueda
  comprimirse con herramientas específicas de la plataforma (ZIP para Windows, archivos comprimidos para
  Unix).
- Captura de metadatos del paquete con `cargo xtask mochi-bundle --matrix dist/matrix.json`
  para que los trabajos de lanzamiento puedan publicar un único índice JSON que enumere cada host/perfil
  artefacto producido en la tubería.
- Utilice `cargo xtask mochi-bundle --stage /mnt/staging/mochi` (o similar) en cada
  crear un agente para cargar el paquete y archivarlo en un directorio compartido que el
  el trabajo de publicación puede consumir.
- Publicar tanto el archivo como `manifest.json` para que los operadores puedan verificar el paquete.
  integridad.
- Almacenar el directorio generado como un artefacto de compilación para generar pruebas de humo que
  Ejercer el supervisor con binarios empaquetados deterministamente.
- Registre los hashes del paquete en las notas de la versión o en el registro `status.md` para el futuro.
  controles de procedencia.