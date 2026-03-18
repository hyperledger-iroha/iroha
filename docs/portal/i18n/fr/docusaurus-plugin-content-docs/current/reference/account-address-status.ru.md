---
lang: fr
direction: ltr
source: docs/portal/docs/reference/account-address-status.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : compte-adresse-statut
titre : Соответствие адресов аккаунтов
description : Le processus de développement du luminaire ADDR-2 et la synchronisation du SDK de commande.
---

Le paquet canonique ADDR-2 (`fixtures/account/address_vectors.json`) comprend les appareils I105 (de préférence), compressé (`sora`, deuxième meilleur ; demi/pleine largeur), multisignature et négatif. Vous pouvez utiliser SDK + Torii pour utiliser Odin et JSON afin d'exploiter votre codec dans votre produit. Cette page indique le statut de l'entreprise (`docs/source/account_address_status.md` dans le référentiel de fichiers), ce que le portail peut utiliser pour le flux de travail Il n'y a aucun moyen de s'acheter en mono-repo.

## Prégénération ou livraison de paquets

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Drapeaux :

- `--stdout` — JSON sur la sortie standard pour des tests ad hoc.
- `--out <path>` — записывает в другой путь (par exemple, при локальном сравнении изменений).
- `--verify` — сравнивает рабочую копию со свежесгенерированным содержимым (neльзя совмещать с `--stdout`).

Flux de travail CI ** Dérive du vecteur d'adresse ** запускает `cargo xtask address-vectors --verify`
Ici, vous trouverez des appareils, des générateurs ou des documents, qui ne devraient pas prédire les publications.

## Comment utiliser un luminaire ?

| Surfaces | Validation |
|--------------|------------|
| Modèle de données Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (serveur) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK Swift | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |Ce harnais permet d'aller-retour en canonique avec un bateau + I105 + un code et une vérification, ce code pour le style Norito est adapté au luminaire для négatif кейсов.

## Est-il possible d'automatiser ?

L'outil de libération peut automatiser la mise à jour du luminaire grâce à l'aide
`scripts/account_fixture_helper.py`, que vous avez trouvé ou fourni un paquet canonique sans copie/installation :

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

Helper propose des remplacements pour `--source` ou pour l'option `IROHA_ACCOUNT_FIXTURE_URL`, que le CI du SDK peut utiliser avant l'utilisation. Avant que l'assistant `--metrics-out` n'installe `account_address_fixture_check_status{target="…"}` dans le résumé canonique SHA-256 (`account_address_fixture_remote_info`), vous trouverez les collecteurs de fichiers texte Prometheus et Le bord Grafana `account_address_fixture_status` peut améliorer la synchronisation selon les normes. Alerte, votre cible correspond à `0`. Pour l'automatisation multi-surfaces, utilisez le wrapper `ci/account_fixture_metrics.sh` (pour `--target label=path[::source]`), les commandes d'astreinte publiées `.prom` est destiné au nœud-exportateur de collecteur de fichiers texte.

## Votre bracelet polaire ?

Полный STATUS соответствия ADDR-2 (propriétaires, plan de surveillance, открытые задачи)
Vous trouverez dans le référentiel `docs/source/account_address_status.md` la structure d'adresse RFC (`docs/account_structure.md`). Utilisez cette section pour les informations opérationnelles ; для глубокой справки обращайтесь к docs репозитория.