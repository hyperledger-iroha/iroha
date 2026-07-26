---
lang: pt
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-11-29T19:27:21.735343+00:00"
translation_last_reviewed: 2026-01-01
---

# Imagem do builder Docker

Este contenedor e definido em `Dockerfile.build` e agrupa todas as dependencias do toolchain
necessarias para CI e builds locais de release. A imagem agora roda como usuario sem root por
padrao, entao as operacoes Git continuam funcionando com o pacote `libgit2` do Arch Linux sem
recorrer ao workaround global `safe.directory`.

## Argumentos de build

- `BUILDER_USER` - nome de login criado dentro do contenedor (padrao: `iroha`).
- `BUILDER_UID` - id de usuario numerico (padrao: `1000`).
- `BUILDER_GID` - id de grupo primario (padrao: `1000`).

Quando voce montar o workspace a partir do host, passe valores UID/GID correspondentes para que
os artefatos gerados continuem gravaveis:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

Os diretorios do toolchain (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`) sao de
propriedade do usuario configurado para que os comandos Cargo, rustup e Poetry continuem
funcionando quando o contenedor perde privilegios de root.

## Executando builds

Anexe seu workspace a `/workspace` (o `WORKDIR` do contenedor) ao invocar a imagem. Exemplo:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

A imagem mantem a membresia do grupo `docker` para que comandos Docker aninhados (p. ex.
`docker buildx bake`) continuem disponiveis para workflows de CI que montam o PID e o socket do
host. Ajuste os mapeamentos de grupo conforme o seu ambiente.

## Artefatos Iroha 2 vs Iroha 3

O workspace agora emite binarios separados por linha de release para evitar colisoes:
`iroha3`/`iroha3d` (padrao) e `iroha2`/`iroha2d` (Iroha 2). Use os helpers para produzir o par
necessario:

- `make build` (ou `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) para Iroha 3
- `make build-i2` (ou `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`) para Iroha 2

O seletor fixa os conjuntos de features (`telemetry` + `schema-endpoint` mais a flag de linha
`build-i{2,3}`) para que builds de Iroha 2 nao capturem por engano defaults apenas de Iroha 3.

Bundles de release gerados via `scripts/build_release_bundle.sh` escolhem automaticamente os
nomes corretos dos binarios quando `--profile` e definido como `iroha2` ou `iroha3`.
