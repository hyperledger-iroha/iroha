---
lang: fr
direction: ltr
source: docs/portal/docs/reference/account-address-status.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : compte-adresse-statut
titre : Conformité des adresses de compte
description : Reprise du workflow du luminaire ADDR-2 et de la synchronisation des équipes SDK.
---

Le bundle canonique ADDR-2 (`fixtures/account/address_vectors.json`) capture des luminaires IH58 (de préférence), compressé (`sora`, second-best; demi/pleine largeur), multisignature et négatifs. Chaque surface SDK + Torii s'appuie sur le meme JSON afin de détecter toute dérive de codec avant la production. Cette page reflète le bref de statut interne (`docs/source/account_address_status.md` dans le dépôt racine) pour que les lecteurs du portail puissent consulter le workflow sans rechercher le mono-repo.

## Regenerer ou vérifier le bundle

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Drapeaux :

- `--stdout` - emet le JSON vers sortie standard pour inspection ad hoc.
- `--out <path>` - écrire vers un autre chemin (par ex. lors de la comparaison locale).
- `--verify` - comparer la copie de travail avec un contenu fraîchement générique (ne peut pas être combiné avec `--stdout`).

Le workflow CI **Address Vector Drift** exécute `cargo xtask address-vectors --verify`
chaque fois que le luminaire, le générateur ou les docs changent afin d'alerter immédiatement les réviseurs.

## Qui consomme le luminaire ?| Surfaces | Validation |
|--------------|------------|
| Modèle de données Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (serveur) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK Swift | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Chaque harnais fait un aller-retour des octets canoniques + IH58 + encodages compressés et vérifie que les codes d'erreur de style Norito correspondent au montage pour les cas négatifs.

## Besoin d'automatisation ?

Les outils de release peuvent automatiser les rafraichissements de luminaires avec le helper
`scripts/account_fixture_helper.py`, qui récupère ou vérifie le bundle canonique sans copieur/coller :

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

Le helper accepte les overrides `--source` ou la variable d'environnement `IROHA_ACCOUNT_FIXTURE_URL` pour que les jobs CI du SDK pointent vers leur miroir préféré. Lorsque `--metrics-out` est fourni, le helper ecrit `account_address_fixture_check_status{target="..."}` ainsi que le digest SHA-256 canonique (`account_address_fixture_remote_info`) afin que les collecteurs de fichiers texte Prometheus et le tableau de bord Grafana `account_address_fixture_status` puissent prouver que chaque surface reste synchronisé. Alertez des qu'une cible rapporte `0`. Pour l'automatisation multi-surface, utilisez le wrapper `ci/account_fixture_metrics.sh` (accepte les répétitions `--target label=path[::source]`) afin que les équipes d'astreinte publient un seul fichier `.prom` consolidé pour le collecteur de fichiers texte de node-exporter.

## Besoin du brief complet ?Le statut complet de conformité ADDR-2 (propriétaires, plan de surveillance, actions ouvertes)
se trouve dans `docs/source/account_address_status.md` au sein du dépôt, ainsi que l'Address Structure RFC (`docs/account_structure.md`). Utilisez cette page comme rappel opérationnel rapide; référez-vous aux docs du repo pour un guide approfondi.