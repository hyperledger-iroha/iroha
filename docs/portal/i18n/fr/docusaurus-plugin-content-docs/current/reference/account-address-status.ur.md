---
lang: fr
direction: ltr
source: docs/portal/docs/reference/account-address-status.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : compte-adresse-statut
titre : اکاؤنٹ ایڈریس تعمیل
description : ADDR-2 luminaire est disponible et SDK est disponible en ligne.
---

bundle ADDR-2 canonique (`fixtures/account/address_vectors.json`) I105 (préféré), compressé (`sora`, deuxième meilleur ; demi/pleine largeur), multisignature, appareils négatifs et capture d'image Le SDK + Torii surface et JSON sont compatibles avec la dérive du codec et la dérive du codec est disponible. سکے۔ یہ صفحہ اندرونی status brief (`docs/source/account_address_status.md` ریپوزٹری روٹ میں) et miroir کرتا ہے تاکہ lecteurs de portail pour mono-repo میں کھوج Flux de travail en cours

## Bundle pour régénérer et vérifier

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Drapeaux :

- `--stdout` — inspection ad hoc en JSON et sortie standard pour émettre des fichiers
- `--out <path>` — Chemin d'accès vers le chemin d'accès (chemin d'accès différent)
- `--verify` — copie de travail pour générer du contenu et comparer les fichiers (`--stdout` pour combiner les deux)

Flux de travail CI **Dérive du vecteur d'adresse** `cargo xtask address-vectors --verify`
Il y a un générateur de luminaires et un générateur de documents pour les critiques et les alertes pour les utilisateurs.

## Luminaire کون استعمال کرتا ہے؟

| Surfaces | Validation |
|--------------|------------|
| Modèle de données Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (serveur) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK Swift | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |exploiter les octets canoniques + I105 + encodages compressés (`sora`, deuxième meilleur) pour les codes aller-retour et les cas négatifs pour les codes d'erreur de style Norito et les appareils qui correspondent aux codes d'erreur ہے۔

## Automatisation چاہئے؟

Le dispositif d'outillage de sortie actualise le script `scripts/account_fixture_helper.py` pour la récupération du bundle canonique et vérifie les éléments :

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

L'assistant `--source` remplace la variable d'environnement `IROHA_ACCOUNT_FIXTURE_URL` pour créer des tâches SDK CI et créer un miroir pour créer un miroir. سکیں۔ Le `--metrics-out` est un assistant `account_address_fixture_check_status{target="…"}` qui est un résumé canonique SHA-256 (`account_address_fixture_remote_info`) pour le résumé. Collecteurs de fichiers texte Prometheus et tableau de bord Grafana `account_address_fixture_status` pour la synchronisation de surface et le tableau de bord La cible `0` est en train d'être alertée automatisation multi-surfaces wrapper `ci/account_fixture_metrics.sh` pour le collecteur de fichiers texte (répétable `--target label=path[::source]` pour les équipes d'astreinte) collecteur de fichiers texte exportateur de nœuds pour les équipes de garde consolidé `.prom` publié en ligne

## مکمل bref چاہئے؟

Statut de conformité ADDR-2 (propriétaires, plan de surveillance, éléments d'action en cours)
La structure d'adresse RFC (`docs/account_structure.md`) est basée sur la structure d'adresse RFC (`docs/account_structure.md`). اس صفحہ کو rappel opérationnel rapide کے طور پر استعمال کریں؛ Documents de dépôt de documents de repo gratuits