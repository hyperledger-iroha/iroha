---
lang: es
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-11-29T19:27:21.735343+00:00"
translation_last_reviewed: 2026-01-01
---

# Imagen del builder de Docker

Este contenedor se define en `Dockerfile.build` y agrupa todas las dependencias del toolchain
necesarias para CI y builds locales de release. La imagen ahora se ejecuta como usuario sin root
por defecto, por lo que las operaciones Git siguen funcionando con el paquete `libgit2` de Arch
Linux sin recurrir al workaround global `safe.directory`.

## Argumentos de build

- `BUILDER_USER` - nombre de inicio de sesion creado dentro del contenedor (por defecto: `iroha`).
- `BUILDER_UID` - id de usuario numerico (por defecto: `1000`).
- `BUILDER_GID` - id de grupo primario (por defecto: `1000`).

Cuando montes el workspace desde tu host, pasa los valores UID/GID correspondientes para que los
artefactos generados sigan siendo escribibles:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

Los directorios del toolchain (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`)
pertenecen al usuario configurado para que los comandos de Cargo, rustup y Poetry sigan
funcionando cuando el contenedor deja los privilegios de root.

## Ejecucion de builds

Adjunta tu workspace a `/workspace` (el `WORKDIR` del contenedor) al invocar la imagen. Ejemplo:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

La imagen mantiene la membresia del grupo `docker` para que los comandos Docker anidados (p. ej.
`docker buildx bake`) sigan disponibles para flujos de CI que montan el PID y el socket del host.
Ajusta los mapeos de grupos segun tu entorno.

## Artefactos Iroha 2 vs Iroha 3

El workspace ahora emite binarios separados por linea de release para evitar colisiones:
`iroha3`/`iroha3d` (por defecto) y `iroha2`/`iroha2d` (Iroha 2). Usa los helpers para
producir el par deseado:

- `make build` (o `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) para Iroha 3
- `make build-i2` (o `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`) para Iroha 2

El selector fija los conjuntos de features (`telemetry` + `schema-endpoint` mas el flag
especifico de linea `build-i{2,3}`) para que los builds de Iroha 2 no adopten por accidente
los defaults de solo Iroha 3.

Los bundles de release creados via `scripts/build_release_bundle.sh` eligen automaticamente los
nombres binarios correctos cuando `--profile` se establece en `iroha2` o `iroha3`.
