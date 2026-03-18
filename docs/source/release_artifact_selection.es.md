---
lang: es
direction: ltr
source: docs/source/release_artifact_selection.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3ea92fbfd7a44cd789ecf187e0edc0dcb33969d45836dd55af706424c66656b
source_last_modified: "2025-11-02T04:40:39.806222+00:00"
translation_last_reviewed: 2026-01-01
---

# Seleccion de artefactos de lanzamiento de Iroha

Esta nota aclara que artefactos (bundles e imagenes de contenedor) deben desplegar los operadores para cada perfil de release.

## Perfiles

- **iroha2 (Self-hosted networks)** - configuracion de una sola lane que coincide con `defaults/genesis.json` y `defaults/client.toml`.
- **iroha3 (SORA Nexus)** - configuracion Nexus multi-lane usando plantillas `defaults/nexus/*`.

## Bundles (binarios)

Los bundles se producen via `scripts/build_release_bundle.sh` con `--profile` en `iroha2` o `iroha3`.

Cada tarball contiene:

- `bin/` - `irohad`, `iroha` y `kagami` construidos con el perfil de despliegue.
- `config/` - configuracion de genesis/cliente segun el perfil (single vs. nexus). Los bundles de Nexus incluyen `config.toml` con parametros de lanes y DA.
- `PROFILE.toml` - metadatos que describen perfil, config, version, commit, SO/arquitectura y conjunto de features habilitado.
- Artefactos de metadatos escritos junto al tarball:
  - `<profile>-<version>-<os>.tar.zst`
  - `<profile>-<version>-<os>.tar.zst.sha256`
  - `<profile>-<version>-<os>.tar.zst.sig` y `.pub` (cuando se suministra `--signing-key`)
  - `<profile>-<version>-manifest.json` que captura la ruta del tarball, el hash y los detalles de firma

## Imagenes de contenedor

Las imagenes de contenedor se producen via `scripts/build_release_image.sh` con los mismos argumentos de perfil/config.

Salidas:

- `<profile>-<version>-<os>-image.tar`
- `<profile>-<version>-<os>-image.tar.sha256`
- Firma/clave publica opcional (`*.sig`/`*.pub`)
- `<profile>-<version>-image.json` que registra tag, ID de imagen, hash y metadatos de firma

## Seleccion del artefacto correcto

1. Determine la superficie de despliegue:
   - **SORA Nexus / multi-lane** -> use el bundle e imagen `iroha3`.
   - **Self-hosted single-lane** -> use los artefactos `iroha2`.
   - Si hay duda, ejecute `scripts/select_release_profile.py --network <alias>` o `--chain-id <id>`; el helper mapea redes al perfil correcto segun `release/network_profiles.toml`.
2. Descargue el tarball deseado y los archivos de manifest asociados. Valide el hash SHA256 y la firma antes de desempacar:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub        -signature iroha3-<version>-linux.tar.zst.sig        iroha3-<version>-linux.tar.zst
   ```
3. Extraiga el bundle (`tar --use-compress-program=zstd -xf <tar>`) y coloque `bin/` en el PATH de despliegue. Aplique overrides de configuracion local cuando sea necesario.
4. Cargue la imagen de contenedor con `docker load -i <profile>-<version>-<os>-image.tar` si usa despliegues contenerizados. Verifique el hash/firma como arriba antes de cargar.

## Checklist de configuracion Nexus

- `config/config.toml` debe incluir las secciones `[nexus]`, `[nexus.lane_catalog]`, `[nexus.dataspace_catalog]` y `[nexus.da]`.
- Confirme que las reglas de ruteo de lane coincidan con las expectativas de governance (`nexus.routing_policy`).
- Valide que los umbrales de DA (`nexus.da`) y los parametros de fusion (`nexus.fusion`) esten alineados con los ajustes aprobados por el consejo.

## Checklist de configuracion single-lane

- `config/config.d` (si existe) debe contener solo overrides single-lane, sin secciones `[nexus]`.
- Asegure que `config/client.toml` referencie el endpoint Torii y la lista de peers previstos.
- Genesis debe conservar los dominios/activos canonicos para la red self-hosted.

## Referencia rapida de tooling

- `scripts/build_release_bundle.sh --help`
- `scripts/build_release_image.sh --help`
- `scripts/select_release_profile.py --list`
- `docs/source/sora_nexus_operator_onboarding.md` - flujo de onboarding de extremo a extremo para operadores de data-space de Sora Nexus una vez seleccionados los artefactos.
