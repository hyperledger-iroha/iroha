---
lang: es
direction: ltr
source: docs/source/mochi_bundle.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f2dd292b7d15b449f3cec1b79343387a8c23beef3a163367bd5fa8ced8593aae
source_last_modified: "2026-01-03T18:08:00.656311+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Paquete de herramientas MOCHI

MOCHI se entrega con un flujo de trabajo de empaque liviano para que los desarrolladores puedan producir un
Paquete de escritorio portátil sin cableado de scripts CI personalizados. El `xtask`
El subcomando maneja la compilación, el diseño, el hashing y (opcionalmente) el archivo.
creación de una sola vez.

## Generando un paquete

```bash
cargo xtask mochi-bundle
```

De forma predeterminada, el comando crea archivos binarios de lanzamiento, ensambla el paquete en
`target/mochi-bundle/` y emite un archivo `mochi-<os>-<arch>-release.tar.gz`
junto con un determinista `manifest.json`. El manifiesto enumera todos los archivos con
su tamaño y hash SHA-256 para que las canalizaciones de CI puedan volver a ejecutar la verificación o publicar
atestados. El asistente garantiza que tanto el shell del escritorio `mochi` como el
El binario del espacio de trabajo `kagami` está presente, por lo que la generación de génesis funciona a partir del
caja.

### Banderas

| Bandera | Descripción |
|---------------------|-----------------------------------------------------------------------------|
| `--out <dir>` | Anule el directorio de salida (el valor predeterminado es `target/mochi-bundle`).         |
| `--profile <name>` | Compile con un perfil de carga específico (por ejemplo, `debug` para pruebas).              |
| `--no-archive` | Omita el archivo `.tar.gz` y deje solo la carpeta preparada.               |
| `--kagami <path>` | Utilice un binario `kagami` explícito en lugar de compilar `iroha_kagami`.         |
| `--matrix <path>` | Agregue metadatos del paquete a una matriz JSON para realizar un seguimiento de la procedencia de CI.         |
| `--smoke` | Ejecute `mochi --help` desde el paquete empaquetado como puerta de ejecución básica.      |
| `--stage <dir>` | Copie el paquete terminado (y archívelo, cuando esté presente) en una carpeta provisional. |

`--stage` está destinado a canalizaciones de CI donde cada agente de compilación carga su
artefactos a una ubicación compartida. El asistente recrea el directorio del paquete y
copia el archivo generado en el directorio provisional para que los trabajos de publicación puedan
recopile resultados específicos de la plataforma sin scripts de shell.

El diseño dentro del paquete es intencionalmente simple:

```
bin/mochi              # egui desktop executable
bin/kagami             # kagami helper for genesis generation
config/sample.toml     # starter supervisor configuration
docs/README.md         # bundle overview and verification guide
LICENSE                # repository licence
manifest.json          # generated file manifest with SHA-256 digests
```

### Anulaciones de tiempo de ejecución

El ejecutable `mochi` empaquetado acepta anulaciones de línea de comandos para la mayoría
entornos comunes de supervisor. Utilice estas banderas en lugar de editar
`config/local.toml` al experimentar:

```
./bin/mochi --data-root ./data --profile four-peer-bft \
    --torii-start 12000 --p2p-start 14000 \
    --irohad /path/to/irohad --kagami /path/to/kagami
```

Cualquier valor CLI tiene prioridad sobre las entradas y el entorno `config/local.toml`
variables.

## Automatización de instantáneas

`manifest.json` registra la marca de tiempo de generación, triple objetivo, perfil de carga,
y el inventario completo del expediente. Las canalizaciones pueden diferenciar el manifiesto para detectar cuándo
aparecen nuevos artefactos, cargue el JSON junto con los activos de lanzamiento o audite el
hashes antes de promocionar un paquete a los operadores.

El asistente es idempotente: volver a ejecutar el comando actualiza el manifiesto y
sobrescribe el archivo anterior, manteniendo `target/mochi-bundle/` como único
fuente de verdad para el último paquete en la máquina actual.