---
lang: fr
direction: ltr
source: docs/portal/docs/reference/account-address-status.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : compte-adresse-statut
titre : Cummplimiento de direcciones de cuenta
description : Résumé du travail sur le luminaire ADDR-2 et comme les équipements du SDK sont synchronisés.
---

Le paquet canonique ADDR-2 (`fixtures/account/address_vectors.json`) capture les appareils I105 (préféré), compressé (`sora`, deuxième meilleur ; demi/pleine largeur), multisignature et négatifs. Chaque surface du SDK + Torii est utilisée dans le même JSON pour détecter tout dérivé du codec avant de commencer la production. Cette page reflète le dossier de l'état interne (`docs/source/account_address_status.md` dans le dépôt principal) pour que les lecteurs du portail consultent le flux sans chercher dans le mono-dépôt.

## Régénérer ou vérifier le paquet

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Drapeaux :

- `--stdout` - émet le JSON sur la sortie standard pour une inspection ad hoc.
- `--out <path>` - écrire dans une route différente (par exemple, pour comparer les changements localement).
- `--verify` - comparez la copie de travail avec le contenu reçu généré (ne peut pas être combinée avec `--stdout`).

Le workflow de CI **Address Vector Drift** est exécuté `cargo xtask address-vectors --verify`
Chaque fois que vous modifiez l'appareil, le générateur ou la documentation pour alerter immédiatement les critiques.

## Quien consomme el luminaire ?| Superficie | Validation |
|--------------|------------|
| Modèle de données Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (serveur) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK Swift | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Chaque harnais a un aller-retour de octets canoniques + I105 + codifications compressées et vérifie que les codes d'erreur du style Norito coïncident avec le montage pour les cas négatifs.

## Avez-vous besoin d'une automatisation ?

Les outils de libération peuvent automatiser les fresques des luminaires avec l'assistant
`scripts/account_fixture_helper.py`, pour obtenir ou vérifier le paquet canonique sans étapes de copie/chargement :

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

L'assistant accepte les remplacements de `--source` ou la variable d'entrée `IROHA_ACCOUNT_FIXTURE_URL` pour que les tâches du CI du SDK soient appliquées à votre miroir préféré. Lorsque vous proposez `--metrics-out`, l'aide décrit `account_address_fixture_check_status{target="..."}` avec le résumé SHA-256 canonique (`account_address_fixture_remote_info`) pour les collecteurs de fichiers texte de Prometheus et le tableau de bord de Grafana `account_address_fixture_status` peut vérifier que chaque surface est en synchronisation. Alerte lorsqu'une cible est signalée `0`. Pour l'automatisation multi-superficie, utilisez le wrapper `ci/account_fixture_metrics.sh` (acceptez `--target label=path[::source]` à plusieurs reprises) pour que les équipes publiques d'astreinte créent un fichier unique `.prom` consolidé pour le collecteur de fichiers texte de l'exportateur de nœuds.

## Avez-vous besoin du brief complet ?L'état complet de l'ensemble ADDR-2 (propriétaires, plan de surveillance, actions ouvertes) se trouve dans `docs/source/account_address_status.md` à l'intérieur du référentiel avec la structure d'adresse RFC (`docs/account_structure.md`). Utilisez cette page comme enregistreur opérationnel rapide ; pour vous guider en profondeur, consultez les documents du dépôt.