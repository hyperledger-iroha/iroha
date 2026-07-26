---
lang: fr
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-11-29T19:27:21.735343+00:00"
translation_last_reviewed: 2026-01-01
---

# Image du builder Docker

Ce conteneur est defini dans `Dockerfile.build` et regroupe toutes les dependances
necessaires du toolchain pour la CI et les builds locales de release. L image tourne
maintenant en utilisateur non-root par defaut, donc les operations Git continuent de
fonctionner avec le paquet `libgit2` d Arch Linux sans recourir au contournement global
`safe.directory`.

## Arguments de build

- `BUILDER_USER` - nom de connexion cree dans le conteneur (par defaut: `iroha`).
- `BUILDER_UID` - id utilisateur numerique (par defaut: `1000`).
- `BUILDER_GID` - id de groupe primaire (par defaut: `1000`).

Quand vous montez le workspace depuis l hote, passez des valeurs UID/GID correspondantes
pour que les artefacts generes restent modifiables:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

Les repertoires du toolchain (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`)
sont possedes par l utilisateur configure afin que les commandes Cargo, rustup et Poetry
restent pleinement fonctionnelles une fois que le conteneur quitte les privileges root.

## Lancer des builds

Attachez votre workspace a `/workspace` (le `WORKDIR` du conteneur) lors de l invocation
de l image. Exemple:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

L image conserve l appartenance au groupe `docker` afin que les commandes Docker imbriquees
(p. ex. `docker buildx bake`) restent disponibles pour les workflows CI qui montent le PID
et le socket de l hote. Ajustez les mappings de groupes selon votre environnement.

## Artefacts Iroha 2 vs Iroha 3

Le workspace produit maintenant des binaires separes par ligne de release pour eviter les
collisions: `iroha3`/`iroha3d` (par defaut) et `iroha2`/`iroha2d` (Iroha 2). Utilisez
les helpers pour produire la paire souhaitee:

- `make build` (ou `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) pour Iroha 3
- `make build-i2` (ou `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`) pour Iroha 2

Le selecteur fixe les ensembles de features (`telemetry` + `schema-endpoint` plus le flag
specifique a la ligne `build-i{2,3}`) afin que les builds Iroha 2 ne recuperent pas par
accident les defaults propres a Iroha 3.

Les bundles de release construits via `scripts/build_release_bundle.sh` choisissent
automatiquement les bons noms de binaires lorsque `--profile` est defini sur `iroha2`
ou `iroha3`.
