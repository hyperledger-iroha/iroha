---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56d820b8f976b561f2a28333ad55ed673c3fbfe58fc5152811099774bef85caf
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: account-address-status
title: Conformite des adresses de compte
description: Resume du workflow du fixture ADDR-2 et de la synchronisation des equipes SDK.
---

Le bundle canonique ADDR-2 (`fixtures/account/address_vectors.json`) capture des fixtures canonical Katakana i105, multisignature et negatifs. Chaque surface SDK + Torii s'appuie sur le meme JSON afin de detecter toute derive de codec avant la production. Cette page reflete le brief de statut interne (`docs/source/account_address_status.md` dans le depot racine) pour que les lecteurs du portail puissent consulter le workflow sans fouiller le mono-repo.

## Regenerer ou verifier le bundle

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Flags:

- `--stdout` - emet le JSON vers stdout pour inspection ad-hoc.
- `--out <path>` - ecrit vers un autre chemin (par ex. lors de la comparaison locale).
- `--verify` - compare la copie de travail avec un contenu fraichement genere (ne peut pas etre combine avec `--stdout`).

Le workflow CI **Address Vector Drift** execute `cargo xtask address-vectors --verify`
chaque fois que le fixture, le generateur ou les docs changent afin d'alerter immediatement les reviewers.

## Qui consomme le fixture ?

| Surface | Validation |
|---------|------------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Chaque harness fait un aller-retour des octets canoniques + I105 + encodages compresses et verifie que les codes d'erreur de style Norito correspondent au fixture pour les cas negatifs.

## Besoin d'automatisation ?

Les outils de release peuvent automatiser les rafraichissements de fixtures avec le helper
`scripts/account_fixture_helper.py`, qui recupere ou verifie le bundle canonique sans copier/coller :

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

Le helper accepte des overrides `--source` ou la variable d'environnement `IROHA_ACCOUNT_FIXTURE_URL` pour que les jobs CI des SDK pointent vers leur miroir prefere. Lorsque `--metrics-out` est fourni, le helper ecrit `account_address_fixture_check_status{target="..."}` ainsi que le digest SHA-256 canonique (`account_address_fixture_remote_info`) afin que les textfile collectors Prometheus et le dashboard Grafana `account_address_fixture_status` puissent prouver que chaque surface reste synchronisee. Alertez des qu'une cible rapporte `0`. Pour l'automatisation multi-surface, utilisez le wrapper `ci/account_fixture_metrics.sh` (accepte des `--target label=path[::source]` repetes) afin que les equipes d'astreinte publient un seul fichier `.prom` consolide pour le textfile collector de node-exporter.

## Besoin du brief complet ?

Le statut complet de conformite ADDR-2 (owners, plan de monitoring, actions ouvertes)
se trouve dans `docs/source/account_address_status.md` au sein du depot, ainsi que l'Address Structure RFC (`docs/account_structure.md`). Utilisez cette page comme rappel operationnel rapide; referez-vous aux docs du repo pour un guide approfondi.
