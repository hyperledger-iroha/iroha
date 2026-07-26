---
lang: fr
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-27T18:39:03.379028+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Profils Iroha3

Kagami fournit des préréglages pour les réseaux Iroha 3 afin que les opérateurs puissent tamponner le déterminisme
Genesis se manifeste sans jongler avec les boutons par réseau.

- Profils : `iroha3-dev` (chaîne `iroha3-dev.local`, collecteurs k=1 r=1, graine VRF dérivée de l'identifiant de la chaîne lorsque NPoS est sélectionné), `iroha3-taira` (chaîne `iroha3-taira`, collecteurs k=3 r=3, nécessite `--vrf-seed-hex` lorsque NPoS est sélectionné), `iroha3-nexus` (chaîne `iroha3-nexus`, collecteurs k=5 r=3, nécessite `--vrf-seed-hex` lorsque NPoS est sélectionné).
- Consensus : les réseaux de profils Sora (Nexus + espaces de données) nécessitent NPoS et interdisent les basculements par étapes ; les déploiements Iroha3 autorisés doivent s'exécuter sans profil Sora.
- Génération : `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. Utilisez `--consensus-mode npos` pour Nexus ; `--vrf-seed-hex` n'est valable que pour NPoS (obligatoire pour taira/nexus). Kagami épingle DA/RBC sur la ligne Iroha3 et émet un résumé (chaîne, collecteurs, DA/RBC, graine VRF, empreinte digitale).
- Vérification : `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` rejoue les attentes du profil (identifiant de chaîne, DA/RBC, collecteurs, couverture PoP, empreinte digitale consensuelle). Fournissez `--vrf-seed-hex` uniquement lors de la vérification d’un manifeste NPoS pour taira/nexus.
- Exemples de bundles : les bundles pré-générés sont sous `defaults/kagami/iroha3-{dev,taira,nexus}/` (genesis.json, config.toml, docker-compose.yml, verify.txt, README). Régénérez avec `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]`.
- Mochi : `mochi`/`mochi-genesis` accepte `--genesis-profile <profile>` et `--vrf-seed-hex <hex>` (NPoS uniquement), les transmet à Kagami et imprime le même résumé Kagami sur stdout/stderr lorsqu'un profil est utilisé.

Les bundles intègrent des PoP BLS aux côtés des entrées de topologie afin que `kagami verify` réussisse
hors de la boîte ; ajustez les pairs/ports de confiance dans les configurations selon les besoins pour les applications locales.
la fumée coule.