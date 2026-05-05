---
lang: pt
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-27T18:39:03.379028+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Perfis Iroha3

Kagami envia predefinições para redes Iroha 3 para que as operadoras possam carimbar dados determinísticos
genesis se manifesta sem fazer malabarismos com botões por rede.

- Perfis: `iroha3-dev` (cadeia `iroha3-dev.local`, coletores k=1 r=1, semente VRF derivada do ID da cadeia quando NPoS é selecionado), `iroha3-taira` (cadeia `iroha3-taira`, coletores k=3 r=3, requer `--vrf-seed-hex` quando NPoS é selecionado), `iroha3-nexus` (cadeia `iroha3-nexus`, coletores k=5 r=3, requer `--vrf-seed-hex` quando NPoS é selecionado).
- Consenso: redes de perfil Sora (Nexus + dataspaces) exigem NPoS e não permitem cortes em estágios; implantações Iroha3 autorizadas devem ser executadas sem um perfil Sora.
- Geração: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. Use `--consensus-mode npos` para Nexus; `--vrf-seed-hex` é válido apenas para NPoS (obrigatório para taira/nexus). Kagami fixa DA/RBC na linha Iroha3 e emite um resumo (corrente, coletores, DA/RBC, semente VRF, impressão digital).
- Verificação: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` reproduz as expectativas do perfil (ID da cadeia, DA/RBC, coletores, cobertura PoP, impressão digital de consenso). Forneça `--vrf-seed-hex` somente ao verificar um manifesto NPoS para taira/nexus.
- Pacotes de amostra: pacotes pré-gerados residem em `defaults/kagami/iroha3-{dev,taira,nexus}/` (genesis.json, config.toml, docker-compose.yml, verify.txt, README). Regenere com `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]`.
- Mochi: `mochi`/`mochi-genesis` aceita `--genesis-profile <profile>` e `--vrf-seed-hex <hex>` (somente NPoS), encaminha-os para Kagami e imprime o mesmo resumo Kagami para stdout/stderr quando um perfil é usado.

Os pacotes incorporam PoPs BLS junto com entradas de topologia para que `kagami verify` seja bem-sucedido
fora da caixa; ajuste os peers/portas confiáveis nas configurações conforme necessário para local
a fumaça corre.