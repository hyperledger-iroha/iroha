---
lang: pt
direction: ltr
source: docs/source/release_artifact_selection.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3ea92fbfd7a44cd789ecf187e0edc0dcb33969d45836dd55af706424c66656b
source_last_modified: "2025-11-02T04:40:39.806222+00:00"
translation_last_reviewed: 2026-01-01
---

# Selecao de artefatos de release do Iroha

Esta nota esclarece quais artefatos (bundles e imagens de contenedor) os operadores devem implantar para cada perfil de release.

## Perfis

- **iroha2 (Self-hosted networks)** - configuracao de lane unica que corresponde a `defaults/genesis.json` e `defaults/client.toml`.
- **iroha3 (SORA Nexus)** - configuracao Nexus multi-lane usando templates `defaults/nexus/*`.

## Bundles (binarios)

Bundles sao produzidos via `scripts/build_release_bundle.sh` com `--profile` definido para `iroha2` ou `iroha3`.

Cada tarball contem:

- `bin/` - `irohad`, `iroha` e `kagami` construidos com o perfil de deploy.
- `config/` - configuracao de genesis/cliente especifica do perfil (single vs. nexus). Bundles Nexus incluem `config.toml` com parametros de lanes e DA.
- `PROFILE.toml` - metadados que descrevem perfil, config, versao, commit, SO/arch e conjunto de features habilitado.
- Artefatos de metadados escritos ao lado do tarball:
  - `<profile>-<version>-<os>.tar.zst`
  - `<profile>-<version>-<os>.tar.zst.sha256`
  - `<profile>-<version>-<os>.tar.zst.sig` e `.pub` (quando `--signing-key` e fornecida)
  - `<profile>-<version>-manifest.json` capturando o caminho do tarball, hash e detalhes de assinatura

## Imagens de contenedor

Imagens de contenedor sao produzidas via `scripts/build_release_image.sh` com os mesmos argumentos de perfil/config.

Saidas:

- `<profile>-<version>-<os>-image.tar`
- `<profile>-<version>-<os>-image.tar.sha256`
- Assinatura/chave publica opcional (`*.sig`/`*.pub`)
- `<profile>-<version>-image.json` registrando tag, ID da imagem, hash e metadados de assinatura

## Selecionando o artefato correto

1. Determine a superficie de deploy:
   - **SORA Nexus / multi-lane** -> use o bundle e a imagem `iroha3`.
   - **Self-hosted single-lane** -> use os artefatos `iroha2`.
   - Em caso de duvida, execute `scripts/select_release_profile.py --network <alias>` ou `--chain-id <id>`; o helper mapeia redes para o perfil correto via `release/network_profiles.toml`.
2. Baixe o tarball desejado e os arquivos de manifest associados. Valide o hash SHA256 e a assinatura antes de desempacotar:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub        -signature iroha3-<version>-linux.tar.zst.sig        iroha3-<version>-linux.tar.zst
   ```
3. Extraia o bundle (`tar --use-compress-program=zstd -xf <tar>`) e coloque `bin/` no PATH de deploy. Aplique overrides de configuracao local quando necessario.
4. Carregue a imagem de contenedor com `docker load -i <profile>-<version>-<os>-image.tar` se estiver usando deploys conteinerizados. Verifique o hash/assinatura como acima antes de carregar.

## Checklist de configuracao Nexus

- `config/config.toml` deve incluir as secoes `[nexus]`, `[nexus.lane_catalog]`, `[nexus.dataspace_catalog]` e `[nexus.da]`.
- Confirme que as regras de roteamento de lane correspondem as expectativas de governance (`nexus.routing_policy`).
- Valide que os limites de DA (`nexus.da`) e os parametros de fusao (`nexus.fusion`) estejam alinhados com configuracoes aprovadas pelo conselho.

## Checklist de configuracao single-lane

- `config/config.d` (se presente) deve conter apenas overrides single-lane, sem secoes `[nexus]`.
- Garanta que `config/client.toml` aponte para o endpoint Torii e a lista de peers desejados.
- Genesis deve manter os dominios/ativos canonicos para a rede self-hosted.

## Referencia rapida de tooling

- `scripts/build_release_bundle.sh --help`
- `scripts/build_release_image.sh --help`
- `scripts/select_release_profile.py --list`
- `docs/source/sora_nexus_operator_onboarding.md` - fluxo de onboarding de ponta a ponta para operadores de data-space Sora Nexus uma vez que os artefatos forem selecionados.
