---
lang: fr
direction: ltr
source: docs/source/release_artifact_selection.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3ea92fbfd7a44cd789ecf187e0edc0dcb33969d45836dd55af706424c66656b
source_last_modified: "2025-11-02T04:40:39.806222+00:00"
translation_last_reviewed: 2026-01-01
---

# Selection des artefacts de release Iroha

Cette note precise quels artefacts (bundles et images de conteneur) les operateurs doivent deployer pour chaque profil de release.

## Profils

- **iroha2 (Self-hosted networks)** - configuration a lane unique correspondant a `defaults/genesis.json` et `defaults/client.toml`.
- **iroha3 (SORA Nexus)** - configuration Nexus multi-lane utilisant les templates `defaults/nexus/*`.

## Bundles (binaires)

Les bundles sont produits via `scripts/build_release_bundle.sh` avec `--profile` defini sur `iroha2` ou `iroha3`.

Chaque tarball contient:

- `bin/` - `irohad`, `iroha` et `kagami` construits avec le profil de deploiement.
- `config/` - configuration genesis/client specifique au profil (single vs. nexus). Les bundles Nexus incluent `config.toml` avec des parametres de lane et DA.
- `PROFILE.toml` - metadonnees decrivant le profil, la config, la version, le commit, l OS/arch, et le set de features active.
- Artefacts de metadonnees ecrits a cote du tarball:
  - `<profile>-<version>-<os>.tar.zst`
  - `<profile>-<version>-<os>.tar.zst.sha256`
  - `<profile>-<version>-<os>.tar.zst.sig` et `.pub` (quand `--signing-key` est fourni)
  - `<profile>-<version>-manifest.json` capturant le chemin du tarball, le hash et les details de signature

## Images de conteneur

Les images de conteneur sont produites via `scripts/build_release_image.sh` avec les memes arguments de profil/config.

Sorties:

- `<profile>-<version>-<os>-image.tar`
- `<profile>-<version>-<os>-image.tar.sha256`
- Signature/cle publique optionnelle (`*.sig`/`*.pub`)
- `<profile>-<version>-image.json` enregistrant tag, ID image, hash et metadonnees de signature

## Selection du bon artefact

1. Determinez la surface de deploiement:
   - **SORA Nexus / multi-lane** -> utilisez le bundle et l image `iroha3`.
   - **Self-hosted single-lane** -> utilisez les artefacts `iroha2`.
   - En cas de doute, lancez `scripts/select_release_profile.py --network <alias>` ou `--chain-id <id>`; le helper associe les reseaux au bon profil via `release/network_profiles.toml`.
2. Telechargez le tarball souhaite et les fichiers manifest associes. Validez le hash SHA256 et la signature avant decompression:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub        -signature iroha3-<version>-linux.tar.zst.sig        iroha3-<version>-linux.tar.zst
   ```
3. Extrayez le bundle (`tar --use-compress-program=zstd -xf <tar>`) et placez `bin/` dans le PATH de deploiement. Appliquez les overrides de configuration locale si necessaire.
4. Chargez l image de conteneur avec `docker load -i <profile>-<version>-<os>-image.tar` si vous utilisez des deploiements conteneurises. Verifiez le hash/la signature comme ci-dessus avant chargement.

## Checklist de configuration Nexus

- `config/config.toml` doit inclure les sections `[nexus]`, `[nexus.lane_catalog]`, `[nexus.dataspace_catalog]`, et `[nexus.da]`.
- Confirmez que les regles de routage de lane correspondent aux attentes de governance (`nexus.routing_policy`).
- Validez que les seuils DA (`nexus.da`) et les parametres de fusion (`nexus.fusion`) sont alignes avec les reglages approuves par le conseil.

## Checklist de configuration single-lane

- `config/config.d` (si present) ne doit contenir que des overrides single-lane, sans sections `[nexus]`.
- Assurez-vous que `config/client.toml` reference l endpoint Torii vise et la liste des peers.
- Genesis doit conserver les domaines/actifs canoniques pour le reseau self-hosted.

## Reference rapide tooling

- `scripts/build_release_bundle.sh --help`
- `scripts/build_release_image.sh --help`
- `scripts/select_release_profile.py --list`
- `docs/source/sora_nexus_operator_onboarding.md` - flux d onboarding de bout en bout pour les operateurs de data-space Sora Nexus une fois les artefacts selectionnes.
