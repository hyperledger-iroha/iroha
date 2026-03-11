---
lang: fr
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c1214f4d0ad86449c0ef4b8f8cbaa38fe265bab4afcc2930cd30a57c089e6d7
source_last_modified: "2025-11-15T05:05:33.914289+00:00"
translation_last_reviewed: 2026-01-01
---

## Statut de conformite de l'adresse de compte (ADDR-2)

Statut: Accepte 2026-03-30  
Proprietaires: Equipe modele de donnees / Guilde QA  
Reference roadmap: ADDR-2 - Dual-Format Compliance Suite

### 1. Vue d'ensemble

- Fixture: `fixtures/account/address_vectors.json` (I105 + multisig cas positifs/negatifs).
- Portee: payloads V1 deterministes couvrant implicit-default, Local-12, Global registry et les controleurs multisig avec une taxonomie complete des erreurs.
- Distribution: partage entre Rust data-model, Torii, SDKs JS/TS, Swift et Android; le CI echoue si un consommateur devie.
- Source de verite: le generateur vit dans `crates/iroha_data_model/src/account/address/compliance_vectors.rs` et est expose via `cargo xtask address-vectors`.

### 2. Regeneration et verification

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

Options:

- `--out <path>` - substitution optionnelle lors de la production de bundles ad hoc (par defaut `fixtures/account/address_vectors.json`).
- `--stdout` - emet du JSON sur stdout au lieu d'ecrire sur disque.
- `--verify` - compare le fichier actuel au contenu fraichement genere (echoue vite en cas de drift; ne peut pas etre combine avec `--stdout`).

### 3. Matrice des artefacts

| Surface | Application | Notes |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | Analyse le JSON, reconstruit les payloads canoniques et verifie les conversions I105/canonical + les erreurs structurees. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Valide les codecs cote serveur pour que Torii refuse deterministiquement les payloads I105 malformes. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Replique les fixtures V1 (I105/fullwidth) et confirme les codes d'erreur style Norito pour chaque cas negatif. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | Exerce le decodage I105, les payloads multisig et la remontee des erreurs sur les plateformes Apple. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Garantit que les bindings Kotlin/Java restent alignes sur le fixture canonique. |

### 4. Suivi et travail restant

- Rapport de statut: ce document est reference depuis `status.md` et le roadmap pour que les revues hebdomadaires verifient la sante du fixture.
- Resume portail developpeurs: voir **Reference -> Account address compliance** dans le portail docs (`docs/portal/docs/reference/account-address-status.md`).
- Prometheus et dashboards: quand vous verifiez une copie SDK, lancez le helper avec `--metrics-out` (et optionnellement `--metrics-label`) afin que le textfile collector Prometheus ingere `account_address_fixture_check_status{target=...}`. Le dashboard Grafana **Account Address Fixture Status** (`dashboards/grafana/account_address_fixture_status.json`) affiche les comptes pass/fail par surface et expose le digest SHA-256 canonique comme preuve d'audit. Alerter quand une cible rapporte `0`.
- Torii metrics: `torii_address_domain_total{endpoint,domain_kind}` emet desormais pour chaque account literal analyse avec succes, en miroir de `torii_address_invalid_total`/`torii_address_local8_total`. Alerter sur tout trafic `domain_kind="local12"` en production et dupliquer les compteurs dans le dashboard SRE `address_ingest` pour que la retraite de Local-12 ait une preuve d'audit.
- Fixture helper: `scripts/account_fixture_helper.py` telecharge ou verifie le JSON canonique pour que l'automatisation des releases SDK puisse recuperer/verifier le bundle sans copier/coller manuel, tout en ecrivant des metriques Prometheus si besoin. Exemple:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \
    --target path/to/sdk/address_vectors.json \
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
    --metrics-label android
  ```

  Le helper ecrit `account_address_fixture_check_status{target="android"} 1` quand la cible correspond, plus les gauges `account_address_fixture_remote_info` / `account_address_fixture_local_info` qui exposent les digests SHA-256. Les fichiers manquants rapportent `account_address_fixture_local_missing`.
  Automation wrapper: appelez `ci/account_fixture_metrics.sh` depuis cron/CI pour emettre un textfile consolide (par defaut `artifacts/account_fixture/address_fixture.prom`). Passez des entrees repetees `--target label=path` (optionnellement ajoutez `::https://mirror/...` par cible pour remplacer la source) pour que Prometheus scrape un seul fichier couvrant chaque copie SDK/CLI. Le workflow GitHub `address-vectors-verify.yml` execute deja ce helper contre le fixture canonique et televerse l'artefact `account-address-fixture-metrics` pour l'ingestion SRE.
