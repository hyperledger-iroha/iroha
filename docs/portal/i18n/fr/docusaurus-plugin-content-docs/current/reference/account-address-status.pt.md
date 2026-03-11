---
lang: fr
direction: ltr
source: docs/portal/docs/reference/account-address-status.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : compte-adresse-statut
titre : Conformidade de enderecos de conta
description : Résumé du flux du luminaire ADDR-2 et comme équipement de SDK ficam synchronisé.
---

Le paquet canonique ADDR-2 (`fixtures/account/address_vectors.json`) capture les appareils I105 (préféré), compressé (`sora`, deuxième meilleur ; demi/pleine largeur), multisignature et négatif. Chaque surface du SDK + Torii utilise le fichier JSON pour détecter toute dérive du codec avant de télécharger la production. Cette page affiche le bref statut interne (`docs/source/account_address_status.md` dans le référentiel principal) pour que les lecteurs du portail consultent le flux sem vasculhar ou le mono-repo.

## Régénérer ou vérifier le paquet

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Drapeaux :

- `--stdout` - émet du JSON sur la sortie standard pour une inspection ad hoc.
- `--out <path>` - grave sur un chemin différent (ex. : pour comparer les changements localement).
- `--verify` - comparer la copie de travail avec le contenu reçu (il peut être combiné avec `--stdout`).

Le flux de travail de CI **Address Vector Drift** est `cargo xtask address-vectors --verify`
Toujours que le luminaire, le générateur ou les documents soient utilisés pour alerter immédiatement les réviseurs.

## Quem consome o luminaire ?

| Surfaces | Validation |
|--------------|------------|
| Modèle de données Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (serveur) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK Swift | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |Chaque harnais permet un aller-retour de octets canoniques + I105 + encodages compressés et vérifie les codes d'erreur dans le style Norito avec le luminaire pour les cas négatifs.

## Précision de l'automatisation ?

Les ferramentas de release peuvent automatiser l'actualisation du luminaire avec l'aide
`scripts/account_fixture_helper.py`, que je cherche ou vérifie le paquet canonique par copier/coller :

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

L'assistant remplace `--source` ou la variable ambiante `IROHA_ACCOUNT_FIXTURE_URL` pour que les tâches du CI du SDK soient installées pour votre miroir préféré. Lorsque `--metrics-out` est fourni, l'assistant écrit `account_address_fixture_check_status{target=\"...\"}` avec le résumé SHA-256 canonique (`account_address_fixture_remote_info`) pour que les collecteurs de fichiers texte fassent Prometheus et le tableau de bord Grafana. `account_address_fixture_status` prouve que chaque surface est permanente en synchronisation. Soyez alerté lorsque vous signalez la cible `0`. Pour automatiser l'utilisation multi-superficie du wrapper `ci/account_fixture_metrics.sh` (en utilisant `--target label=path[::source]` à plusieurs reprises) pour équiper le public de garde d'un fichier unique `.prom` consolidé pour le collecteur de fichiers texte de l'exportateur de nœuds.

## Précision du bref complet ?

Le statut complet de conformité ADDR-2 (propriétaires, plan de surveillance, éléments de cacao en ouverture)
fica em `docs/source/account_address_status.md` dans le référentiel avec la structure d'adresse RFC (`docs/account_structure.md`). Utilisez cette page comme membre opérationnel rapide ; Pour une orientation détaillée, consultez la documentation du référentiel.